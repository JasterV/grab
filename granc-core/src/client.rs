use crate::{
    BoxError,
    codec::JsonCodec,
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
use futures_util::Stream;
use http_body::Body as HttpBody;
use prost_reflect::{DescriptorError, DescriptorPool, MethodDescriptor};
use std::str::FromStr;
use tokio_stream::StreamExt;
use tonic::{
    metadata::{
        MetadataKey, MetadataValue,
        errors::{InvalidMetadataKey, InvalidMetadataValue},
    },
    transport::{Channel, Endpoint},
};

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

#[derive(thiserror::Error, Debug)]
pub enum GrpcRequestError {
    #[error("Invalid json input: '{0}'")]
    InvalidJson(String),

    #[error("Internal error, the client was not ready: '{0}'")]
    ClientNotReady(#[source] BoxError),

    #[error("Invalid metadata (header) key '{key}': '{source}'")]
    InvalidMetadataKey {
        key: String,
        source: InvalidMetadataKey,
    },
    #[error("Invalid metadata (header) value for key '{key}': '{source}'")]
    InvalidMetadataValue {
        key: String,
        source: InvalidMetadataValue,
    },
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
            .find(|m| m.name() == request.method)
            .ok_or_else(|| DynamicRequestError::MethodNotFound(request.method))?;

        let mut client = tonic::client::Grpc::new(self.service.clone());

        dynamic(&mut client, method, request.body, request.headers)
            .await
            .map_err(DynamicRequestError::from)
    }
}

async fn dynamic<S>(
    client: &mut tonic::client::Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<DynamicResponse, GrpcRequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    match (method.is_client_streaming(), method.is_server_streaming()) {
        (false, false) => {
            let result = unary(client, method, payload, headers).await?;
            Ok(DynamicResponse::Unary(result))
        }

        (false, true) => match server_streaming(client, method, payload, headers).await? {
            Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
            Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
        },
        (true, false) => {
            let input_stream =
                json_array_to_stream(payload).map_err(GrpcRequestError::InvalidJson)?;
            let result = client_streaming(client, method, input_stream, headers).await?;
            Ok(DynamicResponse::Unary(result))
        }

        (true, true) => {
            let input_stream =
                json_array_to_stream(payload).map_err(GrpcRequestError::InvalidJson)?;
            match bidirectional_streaming(client, method, input_stream, headers).await? {
                Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
            }
        }
    }
}

/// Performs a Unary gRPC call (Single Request -> Single Response).
///
/// # Returns
/// * `Ok(Ok(Value))` - Successful RPC execution.
/// * `Ok(Err(Status))` - RPC executed, but server returned an error.
/// * `Err(ClientError)` - Failed to send request or connect.
pub(crate) async fn unary<S>(
    client: &mut tonic::client::Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<Result<serde_json::Value, tonic::Status>, GrpcRequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

    let codec = JsonCodec::new(method.input(), method.output());
    let path = http_path(&method);
    let request = build_request(payload, headers)?;

    match client.unary(request, path, codec).await {
        Ok(response) => Ok(Ok(response.into_inner())),
        Err(status) => Ok(Err(status)),
    }
}

/// Performs a Server Streaming gRPC call (Single Request -> Stream of Responses).
///
/// # Returns
///
/// * `Ok(Ok(Stream))` - Successful RPC execution.
/// * `Ok(Err(Status))` - RPC executed, but server returned an error.
/// * `Err(ClientError)` - Failed to send request or connect.
pub(crate) async fn server_streaming<S>(
    client: &mut tonic::client::Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<
    Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
    GrpcRequestError,
>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

    let codec = JsonCodec::new(method.input(), method.output());
    let path = http_path(&method);
    let request = build_request(payload, headers)?;

    match client.server_streaming(request, path, codec).await {
        Ok(response) => Ok(Ok(response.into_inner())),
        Err(status) => Ok(Err(status)),
    }
}

/// Performs a Client Streaming gRPC call (Stream of Requests -> Single Response).
///
/// # Returns
///
/// * `Ok(Ok(Value))` - Successful RPC execution.
/// * `Ok(Err(Status))` - RPC executed, but server returned an error.
/// * `Err(ClientError)` - Failed to send request or connect.
pub(crate) async fn client_streaming<S>(
    client: &mut tonic::client::Grpc<S>,
    method: MethodDescriptor,
    payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
    headers: Vec<(String, String)>,
) -> Result<Result<serde_json::Value, tonic::Status>, GrpcRequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

    let codec = JsonCodec::new(method.input(), method.output());
    let path = http_path(&method);
    let request = build_request(payload_stream, headers)?;

    match client.client_streaming(request, path, codec).await {
        Ok(response) => Ok(Ok(response.into_inner())),
        Err(status) => Ok(Err(status)),
    }
}

/// Performs a Bidirectional Streaming gRPC call (Stream of Requests -> Stream of Responses).
///
/// # Returns
///
/// * `Ok(Ok(Stream))` - Successful RPC execution.
/// * `Ok(Err(Status))` - RPC executed, but server returned an error.
/// * `Err(ClientError)` - Failed to send request or connect.
async fn bidirectional_streaming<S>(
    client: &mut tonic::client::Grpc<S>,
    method: MethodDescriptor,
    payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
    headers: Vec<(String, String)>,
) -> Result<
    Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
    GrpcRequestError,
>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

    let codec = JsonCodec::new(method.input(), method.output());
    let path = http_path(&method);
    let request = build_request(payload_stream, headers)?;

    match client.streaming(request, path, codec).await {
        Ok(response) => Ok(Ok(response.into_inner())),
        Err(status) => Ok(Err(status)),
    }
}

fn http_path(method: &MethodDescriptor) -> http::uri::PathAndQuery {
    let path = format!("/{}/{}", method.parent_service().full_name(), method.name());
    http::uri::PathAndQuery::from_str(&path).expect("valid gRPC path")
}

fn build_request<T>(
    payload: T,
    headers: Vec<(String, String)>,
) -> Result<tonic::Request<T>, GrpcRequestError> {
    let mut request = tonic::Request::new(payload);
    for (k, v) in headers {
        let key =
            MetadataKey::from_str(&k).map_err(|source| GrpcRequestError::InvalidMetadataKey {
                key: k.clone(),
                source,
            })?;
        let val = MetadataValue::from_str(&v)
            .map_err(|source| GrpcRequestError::InvalidMetadataValue { key: k, source })?;
        request.metadata_mut().insert(key, val);
    }
    Ok(request)
}

fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, String> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err("Client streaming requires a JSON Array body".to_string()),
    }
}
