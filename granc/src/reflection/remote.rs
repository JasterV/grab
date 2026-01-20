use super::generated::reflection_v1::{
    ServerReflectionRequest, server_reflection_client::ServerReflectionClient,
    server_reflection_request::MessageRequest, server_reflection_response::MessageResponse,
};
use crate::reflection::generated::reflection_v1::FileDescriptorResponse;
use crate::reflection::local::LocalReflectionService;
use prost::Message;
use prost_reflect::MethodDescriptor;
use prost_types::{FileDescriptorProto, FileDescriptorSet};
use tonic::transport::{Channel, Endpoint};

pub struct RemoteReflectionService {
    client: ServerReflectionClient<Channel>,
    base_url: String,
}

impl RemoteReflectionService {
    pub async fn connect(base_url: String) -> anyhow::Result<Self> {
        let endpoint =
            Endpoint::new(base_url.clone()).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to {}: {}", base_url, e))?;

        let client = ServerReflectionClient::new(channel);

        Ok(Self { client, base_url })
    }

    pub async fn fetch_method_descriptor(
        &mut self,
        full_method_path: &str,
    ) -> anyhow::Result<MethodDescriptor> {
        let request = ServerReflectionRequest {
            host: self.base_url.clone(),
            message_request: Some(MessageRequest::FileContainingSymbol(
                full_method_path.to_string(),
            )),
        };

        let request_stream = tokio_stream::iter(vec![request]);

        let mut response_stream = self
            .client
            .server_reflection_info(request_stream)
            .await?
            .into_inner();

        // Process the first response
        let Some(response) = response_stream.message().await? else {
            return Err(anyhow::anyhow!("Reflection stream closed without response"));
        };

        let response = match response.message_response {
            Some(MessageResponse::FileDescriptorResponse(descriptor_response)) => {
                descriptor_response
            }
            Some(MessageResponse::ErrorResponse(e)) => {
                return Err(anyhow::anyhow!(
                    "Server returned reflection error: {} (code {})",
                    e.error_message,
                    e.error_code
                ));
            }
            Some(_) => {
                return Err(anyhow::anyhow!(
                    "Received unexpected response type form reflection server"
                ));
            }
            None => return Err(anyhow::anyhow!("Reflection response contained no message")),
        };

        let fd_set = build_file_descriptor_set(response)?;
        let local_reflection_service = LocalReflectionService::from_file_descriptor_set(fd_set)?;

        local_reflection_service
            .fetch_method_descriptor(full_method_path)
            .map_err(From::from)
    }
}

fn build_file_descriptor_set(
    response: FileDescriptorResponse,
) -> Result<FileDescriptorSet, anyhow::Error> {
    let mut fd_set = FileDescriptorSet::default();

    // The server returns raw bytes for each FileDescriptorProto
    for raw_proto in response.file_descriptor_proto {
        let fd = FileDescriptorProto::decode(raw_proto.as_ref())?;
        fd_set.file.push(fd);
    }

    // Make sure we remove duplicates
    fd_set.file.dedup();

    Ok(fd_set)
}
