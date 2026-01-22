mod grpc;

use crate::{
    BoxError,
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
pub use grpc::GrpcRequestError;
use http_body::Body as HttpBody;
use prost_reflect::{DescriptorError, DescriptorPool};
use tonic::transport::{Channel, Endpoint};

#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicRequestError {
    #[error("Invalid input: '{0}'")]
    InvalidInput(String),

    #[error("Failed to read descriptor file: '{0}'")]
    Io(#[from] std::io::Error),

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

pub struct GrpcClient<T = Channel> {
    service: T,
}

impl GrpcClient<Channel> {
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUrl(addr.to_string(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientConnectError::ConnectionFailed(addr.to_string(), e))?;

        Ok(Self { service: channel })
    }
}

impl<S> GrpcClient<S>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    pub async fn dynamic(
        &self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicRequestError> {
        let pool = match request.file_descriptor_set {
            Some(bytes) => DescriptorPool::decode(bytes.as_slice())?,
            // If no proto-set file is passed, we'll try to reach the server reflection service
            None => {
                let mut client = ReflectionClient::new(self.service.clone());
                let fd_set = client
                    .file_descriptor_set_by_symbol(&request.service)
                    .await?;
                DescriptorPool::from_file_descriptor_set(fd_set)?
            }
        };

        let service = pool
            .get_service_by_name(&request.service)
            .ok_or_else(|| DynamicRequestError::ServiceNotFound(request.service))?;

        let method = service
            .methods()
            .find(|m| m.name() == &request.method)
            .ok_or_else(|| DynamicRequestError::MethodNotFound(request.method))?;

        let mut client = tonic::client::Grpc::new(self.service.clone());

        grpc::dynamic(&mut client, method, request.body, request.headers)
            .await
            .map_err(DynamicRequestError::from)
    }
}
