//! # Reflection Client
//!
//! A client implementation for `grpc.reflection.v1`.
//!
//! This client is responsible for building a complete `FileDescriptorSet` by querying
//! a server that supports reflection. It handles the complexity of dependency management by inspecting
//! imports and recursively fetching missing files until the entire schema tree for a
//! requested symbol is resolved.
//!
//! ## References
//!
//! * [gRPC Server Reflection Protocol](https://github.com/grpc/grpc/blob/master/doc/server-reflection.md)//!
use super::generated::reflection_v1::{
    ServerReflectionRequest, ServerReflectionResponse,
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse,
};
use crate::BoxError;
use http_body::Body as HttpBody;
use prost::Message;
use prost_types::{FileDescriptorProto, FileDescriptorSet};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tonic::{Streaming, client::GrpcService};

#[derive(Debug, thiserror::Error)]
pub enum ReflectionResolveError {
    #[error(
        "Failed to start a stream request with the reflection server, reflection might not be supported: '{0}'"
    )]
    ServerStreamInitFailed(#[source] tonic::Status),

    #[error("The server stream returned an error status: '{0}'")]
    ServerStreamFailure(#[source] tonic::Status),

    #[error("Reflection stream closed unexpectedly")]
    StreamClosed,

    #[error("Internal error: Failed to send request to stream")]
    SendFailed,

    #[error("Server returned reflection error code {code}: {message}")]
    ServerError { code: i32, message: String },

    #[error("Protocol error: Received unexpected response type: {0}")]
    UnexpectedResponseType(String),

    #[error("Failed to decode FileDescriptorProto: {0}")]
    DecodeError(#[from] prost::DecodeError),
}

// The host defined in the reflection requests doesn't seem to be a mandatory field
// and there is no documentation about what it is about.
// So we won't enforce it from the user.
const EMPTY_HOST: &str = "";

/// A generic client for the gRPC Server Reflection Protocol.
pub struct ReflectionClient<T = Channel> {
    client: ServerReflectionClient<T>,
}

impl<S> ReflectionClient<S>
where
    S: GrpcService<tonic::body::Body>,
    S::Error: Into<BoxError>,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn new(channel: S) -> Self {
        let client = ServerReflectionClient::new(channel);
        Self { client }
    }

    /// Asks the reflection service for the file containing the requested symbol (e.g., `my.package.MyService`).
    ///
    /// **Recursive Resolution**:
    ///    - The server returns a `FileDescriptorProto`.
    ///    - The client inspects the imports (dependencies) of that file.
    ///    - It recursively requests any missing dependencies until the full `FileDescriptorSet` is built.
    ///
    /// # Returns
    ///
    /// * `Ok(fd_set)` - Successful reflection requests execution.
    /// * `Err(ReflectionResolveError)` - Failed to request file descriptors to the reflection service.
    pub async fn file_descriptor_set_by_symbol(
        &mut self,
        service_name: &str,
    ) -> Result<FileDescriptorSet, ReflectionResolveError> {
        // Initialize Stream
        let (tx, rx) = mpsc::channel(100);

        let mut response_stream = self
            .client
            .server_reflection_info(ReceiverStream::new(rx))
            .await
            .map_err(ReflectionResolveError::ServerStreamInitFailed)?
            .into_inner();

        // Send Initial Request
        let req = ServerReflectionRequest {
            host: EMPTY_HOST.to_string(),
            message_request: Some(MessageRequest::FileContainingSymbol(
                service_name.to_string(),
            )),
        };

        tx.send(req)
            .await
            .map_err(|_| ReflectionResolveError::SendFailed)?;

        // Fetch all transitive dependencies
        let file_map = collect_descriptors(&mut response_stream, tx).await?;

        // Build Registry directly
        let fd_set = FileDescriptorSet {
            file: file_map.into_values().collect(),
        };

        Ok(fd_set)
    }
}

async fn collect_descriptors(
    response_stream: &mut Streaming<ServerReflectionResponse>,
    request_channel: mpsc::Sender<ServerReflectionRequest>,
) -> Result<HashMap<String, FileDescriptorProto>, ReflectionResolveError> {
    let mut inflight = 1;
    let mut collected_files = HashMap::new();
    let mut requested = HashSet::new();

    while inflight > 0 {
        let response = response_stream
            .message()
            .await
            .map_err(ReflectionResolveError::ServerStreamFailure)?
            .ok_or(ReflectionResolveError::StreamClosed)?;

        inflight -= 1;

        match response.message_response {
            Some(MessageResponse::FileDescriptorResponse(res)) => {
                let sent_count = process_descriptor_batch(
                    res.file_descriptor_proto,
                    &mut collected_files,
                    &mut requested,
                    &request_channel,
                )
                .await?;

                inflight += sent_count;
            }
            Some(MessageResponse::ErrorResponse(e)) => {
                return Err(ReflectionResolveError::ServerError {
                    message: e.error_message,
                    code: e.error_code,
                });
            }
            Some(other) => {
                return Err(ReflectionResolveError::UnexpectedResponseType(format!(
                    "{:?}",
                    other
                )));
            }
            None => {
                return Err(ReflectionResolveError::UnexpectedResponseType(
                    "Empty Message".into(),
                ));
            }
        }
    }

    Ok(collected_files)
}

async fn process_descriptor_batch(
    raw_protos: Vec<Vec<u8>>,
    collected_files: &mut HashMap<String, FileDescriptorProto>,
    requested: &mut HashSet<String>,
    tx: &mpsc::Sender<ServerReflectionRequest>,
) -> Result<usize, ReflectionResolveError> {
    let mut sent_count = 0;

    for raw in raw_protos {
        let fd = FileDescriptorProto::decode(raw.as_ref())?;

        if let Some(name) = &fd.name
            && !collected_files.contains_key(name)
        {
            sent_count += queue_dependencies(&fd, collected_files, requested, tx).await?;

            collected_files.insert(name.clone(), fd);
        }
    }

    Ok(sent_count)
}

async fn queue_dependencies(
    fd: &FileDescriptorProto,
    collected_files: &HashMap<String, FileDescriptorProto>,
    requested: &mut HashSet<String>,
    tx: &mpsc::Sender<ServerReflectionRequest>,
) -> Result<usize, ReflectionResolveError> {
    let mut count = 0;

    for dep in &fd.dependency {
        if !collected_files.contains_key(dep) && requested.insert(dep.clone()) {
            let req = ServerReflectionRequest {
                host: EMPTY_HOST.to_string(),
                message_request: Some(MessageRequest::FileByFilename(dep.clone())),
            };

            tx.send(req)
                .await
                .map_err(|_| ReflectionResolveError::SendFailed)?;
            count += 1;
        }
    }

    Ok(count)
}
