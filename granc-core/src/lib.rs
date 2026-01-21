mod codec;
mod grpc_client;
mod reflection;

use futures_util::{Stream, StreamExt};
use grpc_client::{GrpcClient, GrpcClientError};
use http_body::Body as HttpBody;
use prost_reflect::{DescriptorError, DescriptorPool, MethodDescriptor};
use reflection::{ReflectionClient, client::ReflectionResolveError};
use tonic::transport::{Channel, Endpoint};

/// Type alias for the standard boxed error used in generic bounds.
type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GrpcCallError {
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

    #[error("gRPC client error: '{0}'")]
    Client(#[from] GrpcClientError),
}

pub struct GrpcRequest {
    pub file_descriptor_set: Option<Vec<u8>>,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,
    pub service: String,
    pub method: String,
}

pub enum GrpcResponse {
    Unary(Result<serde_json::Value, tonic::Status>),
    Streaming(Result<Vec<Result<serde_json::Value, tonic::Status>>, tonic::Status>),
}

pub struct Granc<T = Channel> {
    service: T,
}

impl Granc<Channel> {
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

impl<S> Granc<S>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    pub async fn call(&self, request: GrpcRequest) -> Result<GrpcResponse, GrpcCallError> {
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
            .ok_or_else(|| GrpcCallError::ServiceNotFound(request.service))?;

        let method = service
            .methods()
            .find(|m| m.name() == &request.method)
            .ok_or_else(|| GrpcCallError::MethodNotFound(request.method))?;

        match (method.is_client_streaming(), method.is_server_streaming()) {
            (false, false) => self.unary(method, request.body, request.headers).await,
            (false, true) => {
                self.server_stream(method, request.body, request.headers)
                    .await
            }
            (true, false) => {
                self.client_stream(method, request.body, request.headers)
                    .await
            }
            (true, true) => {
                self.bidirectional_stream(method, request.body, request.headers)
                    .await
            }
        }
    }

    async fn unary(
        &self,
        method: MethodDescriptor,
        body: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<GrpcResponse, GrpcCallError> {
        let client = GrpcClient::new(self.service.clone());
        let result = client.unary(method, body, headers).await?;
        Ok(GrpcResponse::Unary(result))
    }

    async fn server_stream(
        &self,
        method: MethodDescriptor,
        body: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<GrpcResponse, GrpcCallError> {
        let client = GrpcClient::new(self.service.clone());
        match client.server_streaming(method, body, headers).await? {
            Ok(stream) => Ok(GrpcResponse::Streaming(Ok(stream.collect().await))),
            Err(status) => Ok(GrpcResponse::Streaming(Err(status))),
        }
    }

    async fn client_stream(
        &self,
        method: MethodDescriptor,
        body: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<GrpcResponse, GrpcCallError> {
        let client = GrpcClient::new(self.service.clone());
        let input_stream = json_array_to_stream(body)?;

        let result = client
            .client_streaming(method, input_stream, headers)
            .await?;

        Ok(GrpcResponse::Unary(result))
    }

    async fn bidirectional_stream(
        &self,
        method: MethodDescriptor,
        body: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<GrpcResponse, GrpcCallError> {
        let client = GrpcClient::new(self.service.clone());
        let input_stream = json_array_to_stream(body)?;

        match client
            .bidirectional_streaming(method, input_stream, headers)
            .await?
        {
            Ok(stream) => Ok(GrpcResponse::Streaming(Ok(stream.collect().await))),
            Err(status) => Ok(GrpcResponse::Streaming(Err(status))),
        }
    }
}

fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, GrpcCallError> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err(GrpcCallError::InvalidInput(
            "Client streaming requires a JSON Array body".to_string(),
        )),
    }
}
