//! # gRPC Server Reflection Client
//!
//! This module implements a client for `grpc.reflection.v1`. It enables `granc` to function
//! without a local descriptor file by asking the server for its own schema.
//!
//! ## The Resolution Process
//!
//! 1. **Connect**: Opens a stream to the reflection endpoint.
//! 2. **Request Symbol**: Asks for the file containing the requested service (e.g., `my.package.MyService`).
//! 3. **Recursive Resolution**:
//!    - The server returns a `FileDescriptorProto`.
//!    - The client inspects the imports (dependencies) of that file.
//!    - It recursively requests any missing dependencies until the full schema tree is built.
//! 4. **Build Registry**: Returns a fully populated `DescriptorRegistry`.
//!
use super::generated::reflection_v1::{
    ServerReflectionRequest, ServerReflectionResponse,
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse,
};
use crate::core::BoxError;
use crate::core::reflection::DescriptorRegistry;
use http_body::Body as HttpBody;
use prost::Message;
use prost_types::{FileDescriptorProto, FileDescriptorSet};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tonic::{Streaming, client::GrpcService};

#[cfg(test)]
mod integration_test;

#[derive(Debug, thiserror::Error)]
pub enum ReflectionConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

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

    #[error("Failed to build DescriptorRegistry: {0}")]
    RegistryError(#[from] crate::core::reflection::registry::DescriptorError),
}

/// A generic client for the gRPC Server Reflection Protocol.
pub struct ReflectionClient<T = Channel> {
    client: ServerReflectionClient<T>,
    base_url: String,
}

/// Implementation for the standard network client.
impl ReflectionClient<Channel> {
    pub async fn connect(base_url: String) -> Result<Self, ReflectionConnectError> {
        let endpoint = Endpoint::new(base_url.clone())
            .map_err(|e| ReflectionConnectError::InvalidUrl(base_url.clone(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ReflectionConnectError::ConnectionFailed(base_url.clone(), e))?;

        let client = ServerReflectionClient::new(channel);

        Ok(Self { client, base_url })
    }
}

impl<T> ReflectionClient<T>
where
    T: GrpcService<tonic::body::Body>,
    T::Error: Into<BoxError>,
    T::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <T::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub async fn resolve_service_descriptor_registry(
        &mut self,
        service_name: &str,
    ) -> Result<DescriptorRegistry, ReflectionResolveError> {
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
            host: self.base_url.clone(),
            message_request: Some(MessageRequest::FileContainingSymbol(
                service_name.to_string(),
            )),
        };

        tx.send(req)
            .await
            .map_err(|_| ReflectionResolveError::SendFailed)?;

        // Fetch all transitive dependencies
        let file_map = self.collect_descriptors(&mut response_stream, tx).await?;

        // Build Registry directly
        let fd_set = FileDescriptorSet {
            file: file_map.into_values().collect(),
        };

        Ok(DescriptorRegistry::from_file_descriptor_set(fd_set)?)
    }

    async fn collect_descriptors(
        &self,
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
                    let sent_count = self
                        .process_descriptor_batch(
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
        &self,
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
                sent_count += self
                    .queue_dependencies(&fd, collected_files, requested, tx)
                    .await?;

                collected_files.insert(name.clone(), fd);
            }
        }

        Ok(sent_count)
    }

    async fn queue_dependencies(
        &self,
        fd: &FileDescriptorProto,
        collected_files: &HashMap<String, FileDescriptorProto>,
        requested: &mut HashSet<String>,
        tx: &mpsc::Sender<ServerReflectionRequest>,
    ) -> Result<usize, ReflectionResolveError> {
        let mut count = 0;

        for dep in &fd.dependency {
            if !collected_files.contains_key(dep) && requested.insert(dep.clone()) {
                let req = ServerReflectionRequest {
                    host: self.base_url.clone(),
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
}
