use crate::BoxError;
use crate::codec::JsonCodec;
use futures_util::Stream;
use http_body::Body as HttpBody;
use prost_reflect::MethodDescriptor;
use std::str::FromStr;
use thiserror::Error;
use tokio_stream::StreamExt;
use tonic::{
    Request,
    client::Grpc,
    metadata::{
        MetadataKey, MetadataValue,
        errors::{InvalidMetadataKey, InvalidMetadataValue},
    },
};

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("Invalid input: '{0}'")]
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

pub async fn dynamic<S>(
    client: &mut Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<DynamicGrpcResponse, RequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    match (method.is_client_streaming(), method.is_server_streaming()) {
        (false, false) => {
            let result = unary(client, method, payload, headers).await?;
            Ok(DynamicGrpcResponse::Unary(result))
        }

        (false, true) => match server_streaming(client, method, payload, headers).await? {
            Ok(stream) => Ok(DynamicGrpcResponse::Streaming(Ok(stream.collect().await))),
            Err(status) => Ok(DynamicGrpcResponse::Streaming(Err(status))),
        },
        (true, false) => {
            let input_stream = json_array_to_stream(payload).map_err(RequestError::InvalidJson)?;
            let result = client_streaming(client, method, input_stream, headers).await?;
            Ok(DynamicGrpcResponse::Unary(result))
        }

        (true, true) => {
            let input_stream = json_array_to_stream(payload).map_err(RequestError::InvalidJson)?;
            match bidirectional_streaming(client, method, input_stream, headers).await? {
                Ok(stream) => Ok(DynamicGrpcResponse::Streaming(Ok(stream.collect().await))),
                Err(status) => Ok(DynamicGrpcResponse::Streaming(Err(status))),
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
pub async fn unary<S>(
    client: &mut Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<Result<serde_json::Value, tonic::Status>, RequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| RequestError::ClientNotReady(e.into()))?;

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
pub async fn server_streaming<S>(
    client: &mut Grpc<S>,
    method: MethodDescriptor,
    payload: serde_json::Value,
    headers: Vec<(String, String)>,
) -> Result<
    Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
    RequestError,
>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| RequestError::ClientNotReady(e.into()))?;

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
pub async fn client_streaming<S>(
    client: &mut Grpc<S>,
    method: MethodDescriptor,
    payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
    headers: Vec<(String, String)>,
) -> Result<Result<serde_json::Value, tonic::Status>, RequestError>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| RequestError::ClientNotReady(e.into()))?;

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
pub async fn bidirectional_streaming<S>(
    client: &mut Grpc<S>,
    method: MethodDescriptor,
    payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
    headers: Vec<(String, String)>,
) -> Result<
    Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
    RequestError,
>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError>,
{
    client
        .ready()
        .await
        .map_err(|e| RequestError::ClientNotReady(e.into()))?;

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
) -> Result<Request<T>, RequestError> {
    let mut request = Request::new(payload);
    for (k, v) in headers {
        let key = MetadataKey::from_str(&k).map_err(|source| RequestError::InvalidMetadataKey {
            key: k.clone(),
            source,
        })?;
        let val = MetadataValue::from_str(&v)
            .map_err(|source| RequestError::InvalidMetadataValue { key: k, source })?;
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
