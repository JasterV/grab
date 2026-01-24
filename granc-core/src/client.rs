//! # Granc Client
//!
//! This module implements the high-level logic for executing dynamic gRPC requests
//! and offers support for reflection operations if the server supports it.
use crate::{
    BoxError,
    grpc::client::{GrpcClient, GrpcRequestError},
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
use futures_util::Stream;
use http_body::Body as HttpBody;
use prost_reflect::{DescriptorError, DescriptorPool, MessageDescriptor, ServiceDescriptor};
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Endpoint};

#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ListServicesError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetServiceDescriptorError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),
    #[error("Service '{0}' not found")]
    ServiceNotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GetMessageDescriptorError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),
    #[error("Message '{0}' not found")]
    MessageNotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicCallError {
    #[error("Invalid input: '{0}'")]
    InvalidInput(String),

    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    #[error("Method '{0}' not found")]
    MethodNotFound(String),

    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),

    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),

    #[error("gRPC client request error: '{0}'")]
    GrpcRequestError(#[from] GrpcRequestError),
}

pub struct DynamicRequest {
    pub file_descriptor_set: Option<Vec<u8>>,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,
    pub service: String,
    pub method: String,
}

pub enum DynamicResponse {
    Unary(Result<serde_json::Value, tonic::Status>),
    Streaming(Result<Vec<Result<serde_json::Value, tonic::Status>>, tonic::Status>),
}

pub struct GrancClient<S = Channel> {
    reflection_client: ReflectionClient<S>,
    grpc_client: GrpcClient<S>,
}

impl GrancClient<Channel> {
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUrl(addr.to_string(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientConnectError::ConnectionFailed(addr.to_string(), e))?;

        Ok(Self::new(channel))
    }
}

impl<S> GrancClient<S>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn new(service: S) -> Self {
        let reflection_client = ReflectionClient::new(service.clone());
        let grpc_client = GrpcClient::new(service);

        Self {
            reflection_client,
            grpc_client,
        }
    }

    /// Fetches the list of available services from the server using reflection.
    pub async fn list_services(&mut self) -> Result<Vec<String>, ListServicesError> {
        self.reflection_client
            .list_services()
            .await
            .map_err(Into::into)
    }

    /// Fetches the descriptor for a specific service using reflection.
    /// This allows inspecting methods and types.
    pub async fn get_service_descriptor(
        &mut self,
        service_name: &str,
    ) -> Result<ServiceDescriptor, GetServiceDescriptorError> {
        let fd_set = self
            .reflection_client
            .file_descriptor_set_by_symbol(service_name)
            .await?;

        DescriptorPool::from_file_descriptor_set(fd_set)?
            .get_service_by_name(service_name)
            .ok_or_else(|| GetServiceDescriptorError::ServiceNotFound(service_name.to_string()))
    }

    /// Fetches the descriptor for a specific message using reflection.
    pub async fn get_message_descriptor(
        &mut self,
        message_name: &str,
    ) -> Result<MessageDescriptor, GetMessageDescriptorError> {
        let fd_set = self
            .reflection_client
            .file_descriptor_set_by_symbol(message_name)
            .await?;

        DescriptorPool::from_file_descriptor_set(fd_set)?
            .get_message_by_name(message_name)
            .ok_or_else(|| GetMessageDescriptorError::MessageNotFound(message_name.to_string()))
    }

    /// This function implements the high-level logic for executing dynamic gRPC requests.
    /// It acts as the bridge between the user's intent (a JSON body and a method name)
    /// and the low-level gRPC transport.
    ///
    /// ## How it works
    ///
    /// 1. **Schema Resolution**: It determines whether to use a provided `FileDescriptorSet`
    ///    or to fetch the schema dynamically using the [`crate::reflection::client::ReflectionClient`].
    /// 2. **Method Lookup**: It validates that the requested service and method exist within
    ///    the resolved schema.
    /// 3. **Dispatch**: It inspects the method descriptor to determine the correct gRPC access
    ///    pattern (Unary, Server Streaming, Client Streaming, or Bidirectional) and routes
    ///    the request accordingly.
    /// 4. **Input Adaptation**: It converts input JSON data into the appropriate stream format
    ///    required by the underlying transport.
    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let pool = match request.file_descriptor_set {
            Some(bytes) => DescriptorPool::decode(bytes.as_slice())?,
            // If no proto-set file is passed, we'll try to reach the server reflection service
            None => {
                let fd_set = self
                    .reflection_client
                    .file_descriptor_set_by_symbol(&request.service)
                    .await?;
                DescriptorPool::from_file_descriptor_set(fd_set)?
            }
        };

        let method = pool
            .get_service_by_name(&request.service)
            .ok_or_else(|| DynamicCallError::ServiceNotFound(request.service))?
            .methods()
            .find(|m| m.name() == request.method)
            .ok_or_else(|| DynamicCallError::MethodNotFound(request.method))?;

        match (method.is_client_streaming(), method.is_server_streaming()) {
            (false, false) => {
                let result = self
                    .grpc_client
                    .unary(method, request.body, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (false, true) => {
                match self
                    .grpc_client
                    .server_streaming(method, request.body, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
            (true, false) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                let result = self
                    .grpc_client
                    .client_streaming(method, input_stream, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (true, true) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                match self
                    .grpc_client
                    .bidirectional_streaming(method, input_stream, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
        }
    }
}

fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, String> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err("Client streaming requires a JSON Array body".to_string()),
    }
}
