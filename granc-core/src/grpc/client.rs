//! # Generic gRPC Client
//!
//! This module wraps a standard `tonic` client to provide a generic interface for
//! gRPC communication. It is agnostic to the specific Protobuf messages being exchanged.
//!
//! ## How it works
//!
//! The [`GrpcClient`] utilizes the [`super::codec::JsonCodec`] to handle serialization.
//! It does not need to know the structure of the data it is sending; it simply ensures
//! the connection is established and passes the `serde_json::Value` and `MethodDescriptor`
//! to the codec.
//!
//! ## Features
//!
//! * **Dynamic Pathing**: Constructs the HTTP/2 path (e.g., `/package.Service/Method`) at runtime.
//! * **Metadata Handling**: Converts standard Rust string tuples into Tonic's `MetadataMap` for headers.
//! * **Access Patterns**: Provides specific methods for Unary, Server Streaming, Client Streaming,
//!   and Bidirectional Streaming calls.
use super::codec::JsonCodec;
use crate::BoxError;
use futures_util::Stream;
use http_body::Body as HttpBody;
use prost_reflect::MethodDescriptor;
use std::str::FromStr;
use tonic::{
    client::GrpcService,
    metadata::{
        MetadataKey, MetadataValue,
        errors::{InvalidMetadataKey, InvalidMetadataValue},
    },
    transport::Channel,
};

#[derive(thiserror::Error, Debug)]
pub enum GrpcRequestError {
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

/// A generic client for the gRPC Server Reflection Protocol.
pub struct GrpcClient<S = Channel> {
    client: tonic::client::Grpc<S>,
}

impl<S> GrpcClient<S>
where
    S: GrpcService<tonic::body::Body>,
    S::Error: Into<BoxError>,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn new(service: S) -> Self {
        let client = tonic::client::Grpc::new(service);
        Self { client }
    }

    /// Performs a Unary gRPC call (Single Request -> Single Response).
    ///
    /// # Returns
    /// * `Ok(Ok(Value))` - Successful RPC execution.
    /// * `Ok(Err(Status))` - RPC executed, but server returned an error.
    /// * `Err(ClientError)` - Failed to send request or connect.
    pub async fn unary(
        &mut self,
        method: MethodDescriptor,
        payload: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<Result<serde_json::Value, tonic::Status>, GrpcRequestError> {
        self.client
            .ready()
            .await
            .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload, headers)?;

        match self.client.unary(request, path, codec).await {
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
    pub async fn server_streaming(
        &mut self,
        method: MethodDescriptor,
        payload: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<
        Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
        GrpcRequestError,
    > {
        self.client
            .ready()
            .await
            .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload, headers)?;

        match self.client.server_streaming(request, path, codec).await {
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
    pub async fn client_streaming(
        &mut self,
        method: MethodDescriptor,
        payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
        headers: Vec<(String, String)>,
    ) -> Result<Result<serde_json::Value, tonic::Status>, GrpcRequestError> {
        self.client
            .ready()
            .await
            .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload_stream, headers)?;

        match self.client.client_streaming(request, path, codec).await {
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
    pub async fn bidirectional_streaming(
        &mut self,
        method: MethodDescriptor,
        payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
        headers: Vec<(String, String)>,
    ) -> Result<
        Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
        GrpcRequestError,
    > {
        self.client
            .ready()
            .await
            .map_err(|e| GrpcRequestError::ClientNotReady(e.into()))?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload_stream, headers)?;

        match self.client.streaming(request, path, codec).await {
            Ok(response) => Ok(Ok(response.into_inner())),
            Err(status) => Ok(Err(status)),
        }
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
