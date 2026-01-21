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
use crate::core::reflection::DescriptorRegistry;
use anyhow::Context;
use prost::Message;
use prost_types::{FileDescriptorProto, FileDescriptorSet};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Streaming;
use tonic::transport::{Channel, Endpoint};

pub struct ReflectionClient {
    client: ServerReflectionClient<Channel>,
    base_url: String,
}

impl ReflectionClient {
    pub async fn connect(base_url: String) -> anyhow::Result<Self> {
        let endpoint =
            Endpoint::new(base_url.clone()).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to {}: {}", base_url, e))?;

        Ok(Self {
            client: ServerReflectionClient::new(channel),
            base_url,
        })
    }

    /// Queries the server for a specific service and all its transitive dependencies.
    /// Returns a fully-populated `DescriptorRegistry`.
    pub async fn get_service_descriptor(
        &mut self,
        service_name: &str,
    ) -> anyhow::Result<DescriptorRegistry> {
        // Initialize Stream
        let (tx, rx) = mpsc::channel(100);

        let mut response_stream = self
            .client
            .clone()
            .server_reflection_info(ReceiverStream::new(rx))
            .await
            .context("Failed to start reflection stream")?
            .into_inner();

        // Send Initial Request
        let req = ServerReflectionRequest {
            host: self.base_url.clone(),
            message_request: Some(MessageRequest::FileContainingSymbol(
                service_name.to_string(),
            )),
        };

        tx.send(req).await?;

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
    ) -> anyhow::Result<HashMap<String, FileDescriptorProto>> {
        let mut inflight = 1;
        let mut collected_files = HashMap::new();
        let mut requested = HashSet::new();

        while inflight > 0 {
            let response = response_stream
                .message()
                .await?
                .ok_or_else(|| anyhow::anyhow!("Stream closed unexpectedly"))?;

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
                    return Err(anyhow::anyhow!(
                        "Server returned reflection error: {} (code {})",
                        e.error_message,
                        e.error_code
                    ));
                }
                Some(other) => {
                    return Err(anyhow::anyhow!(
                        "Received unexpected response type: {:?}",
                        other
                    ));
                }
                None => return Err(anyhow::anyhow!("Reflection response contained no message")),
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
    ) -> anyhow::Result<usize> {
        let mut sent_count = 0;

        for raw in raw_protos {
            let fd = FileDescriptorProto::decode(raw.as_ref())
                .context("Failed to decode FileDescriptorProto")?;

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
    ) -> anyhow::Result<usize> {
        let mut count = 0;

        for dep in &fd.dependency {
            if !collected_files.contains_key(dep) && requested.insert(dep.clone()) {
                let req = ServerReflectionRequest {
                    host: self.base_url.clone(),
                    message_request: Some(MessageRequest::FileByFilename(dep.clone())),
                };

                tx.send(req).await?;
                count += 1;
            }
        }

        Ok(count)
    }
}
