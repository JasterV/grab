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
        stream: &mut Streaming<ServerReflectionResponse>,
        tx: mpsc::Sender<ServerReflectionRequest>,
    ) -> anyhow::Result<HashMap<String, FileDescriptorProto>> {
        let mut inflight = 1;
        let mut file_map = HashMap::new();
        let mut requested = HashSet::new();

        while inflight > 0 {
            let response = stream
                .message()
                .await?
                .ok_or_else(|| anyhow::anyhow!("Stream closed unexpectedly"))?;

            inflight -= 1;

            let sent_count = self
                .handle_response(response, &mut file_map, &mut requested, &tx)
                .await?;

            inflight += sent_count;
        }

        Ok(file_map)
    }

    async fn handle_response(
        &self,
        response: ServerReflectionResponse,
        file_map: &mut HashMap<String, FileDescriptorProto>,
        requested: &mut HashSet<String>,
        tx: &mpsc::Sender<ServerReflectionRequest>,
    ) -> anyhow::Result<usize> {
        match response.message_response {
            Some(MessageResponse::FileDescriptorResponse(res)) => {
                self.process_descriptor_batch(res.file_descriptor_proto, file_map, requested, tx)
                    .await
            }
            Some(MessageResponse::ErrorResponse(e)) => Err(anyhow::anyhow!(
                "Server returned reflection error: {} (code {})",
                e.error_message,
                e.error_code
            )),
            Some(other) => Err(anyhow::anyhow!(
                "Received unexpected response type: {:?}",
                other
            )),
            None => Err(anyhow::anyhow!("Reflection response contained no message")),
        }
    }

    async fn process_descriptor_batch(
        &self,
        raw_protos: Vec<Vec<u8>>,
        file_map: &mut HashMap<String, FileDescriptorProto>,
        requested: &mut HashSet<String>,
        tx: &mpsc::Sender<ServerReflectionRequest>,
    ) -> anyhow::Result<usize> {
        let mut sent_count = 0;

        for raw in raw_protos {
            let fd = FileDescriptorProto::decode(raw.as_ref())
                .context("Failed to decode FileDescriptorProto")?;

            if let Some(name) = &fd.name
                && !file_map.contains_key(name)
            {
                sent_count += self
                    .queue_dependencies(&fd, file_map, requested, tx)
                    .await?;
                file_map.insert(name.clone(), fd);
            }
        }

        Ok(sent_count)
    }

    async fn queue_dependencies(
        &self,
        fd: &FileDescriptorProto,
        file_map: &HashMap<String, FileDescriptorProto>,
        requested: &mut HashSet<String>,
        tx: &mpsc::Sender<ServerReflectionRequest>,
    ) -> anyhow::Result<usize> {
        let mut count = 0;
        for dep in &fd.dependency {
            if !file_map.contains_key(dep) && requested.insert(dep.clone()) {
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
