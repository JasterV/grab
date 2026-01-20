//! # gRPC Client Layer
//!
//! This module handles the low-level networking and `tonic` client construction.
//! It is decoupled from the descriptor logic and strictly focuses on sending
//! requests and receiving responses using the `JsonCodec`.
//!
//! # Error Handling
//!
//! - **`ClientError`**: Represents transport errors (connection refused, DNS resolution),
//!   configuration errors (invalid URI), or usage errors (invalid headers).
//! - **`tonic::Status`**: Represents a successful HTTP interaction where the gRPC server
//!   returned an error code (e.g., `NOT_FOUND`, `UNAUTHENTICATED`).
//!
//! The methods in `GrpcClient` separate these two types of errors by returning
//! `Result<Result<T, Status>, ClientError>`.

use crate::codec::JsonCodec;
use futures_util::Stream;
use prost_reflect::MethodDescriptor;
use std::str::FromStr;
use thiserror::Error;
use tonic::{
    Request,
    client::Grpc,
    metadata::{
        MetadataKey, MetadataValue,
        errors::{InvalidMetadataKey, InvalidMetadataValue},
    },
    transport::Channel,
};

/// Represents failures that occur *before* or during the establishment of the network call,
/// or protocol violations that prevent a response.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Invalid uri '{addr}' provided: '{source}'")]
    InvalidUri {
        addr: String,
        source: http::uri::InvalidUri,
    },
    #[error("Failed to connect: '{0}'")]
    ConnectionFailed(tonic::transport::Error),
    #[error("Internal error, the client was not ready: '{0}'")]
    ClientNotReady(tonic::transport::Error),
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

/// A generic gRPC client that uses dynamic dispatch via `prost-reflect`.
pub struct GrpcClient {
    channel: Channel,
}

impl GrpcClient {
    /// Connects to the specified gRPC server address.
    ///
    /// # Arguments
    /// * `addr` - The URI of the server (e.g., `http://localhost:50051`).
    pub async fn connect(addr: &str) -> Result<Self, ClientError> {
        let uri =
            tonic::transport::Uri::from_str(addr).map_err(|source| ClientError::InvalidUri {
                addr: addr.to_string(),
                source,
            })?;

        let channel = Channel::builder(uri)
            .connect()
            .await
            .map_err(ClientError::ConnectionFailed)?;
        Ok(Self { channel })
    }

    /// Performs a Unary gRPC call (Single Request -> Single Response).
    ///
    /// # Returns
    /// * `Ok(Ok(Value))` - Successful RPC execution.
    /// * `Ok(Err(Status))` - RPC executed, but server returned an error.
    /// * `Err(ClientError)` - Failed to send request or connect.
    pub async fn unary(
        &self,
        method: MethodDescriptor,
        payload: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<Result<serde_json::Value, tonic::Status>, ClientError> {
        let mut client = Grpc::new(self.channel.clone());
        client.ready().await.map_err(ClientError::ClientNotReady)?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload, headers)?;

        match client.unary(request, path, codec).await {
            Ok(response) => Ok(Ok(response.into_inner())),
            Err(status) => Ok(Err(status)),
        }
    }

    /// Performs a Server Streaming gRPC call (Single Request -> Stream of Responses).
    pub async fn server_streaming(
        &self,
        method: MethodDescriptor,
        payload: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<
        Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
        ClientError,
    > {
        let mut client = Grpc::new(self.channel.clone());
        client.ready().await.map_err(ClientError::ClientNotReady)?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload, headers)?;

        match client.server_streaming(request, path, codec).await {
            Ok(response) => Ok(Ok(response.into_inner())),
            Err(status) => Ok(Err(status)),
        }
    }

    /// Performs a Client Streaming gRPC call (Stream of Requests -> Single Response).
    pub async fn client_streaming(
        &self,
        method: MethodDescriptor,
        payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
        headers: Vec<(String, String)>,
    ) -> Result<Result<serde_json::Value, tonic::Status>, ClientError> {
        let mut client = Grpc::new(self.channel.clone());
        client.ready().await.map_err(ClientError::ClientNotReady)?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload_stream, headers)?;

        match client.client_streaming(request, path, codec).await {
            Ok(response) => Ok(Ok(response.into_inner())),
            Err(status) => Ok(Err(status)),
        }
    }

    /// Performs a Bidirectional Streaming gRPC call (Stream of Requests -> Stream of Responses).
    pub async fn bidirectional_streaming(
        &self,
        method: MethodDescriptor,
        payload_stream: impl Stream<Item = serde_json::Value> + Send + 'static,
        headers: Vec<(String, String)>,
    ) -> Result<
        Result<impl Stream<Item = Result<serde_json::Value, tonic::Status>>, tonic::Status>,
        ClientError,
    > {
        let mut client = Grpc::new(self.channel.clone());
        client.ready().await.map_err(ClientError::ClientNotReady)?;

        let codec = JsonCodec::new(method.input(), method.output());
        let path = http_path(&method);
        let request = build_request(payload_stream, headers)?;

        match client.streaming(request, path, codec).await {
            Ok(response) => Ok(Ok(response.into_inner())),
            Err(status) => Ok(Err(status)),
        }
    }
}

fn http_path(method: &MethodDescriptor) -> http::uri::PathAndQuery {
    let path = format!("/{}/{}", method.parent_service().full_name(), method.name());
    http::uri::PathAndQuery::from_str(&path).expect("valid gRPC path")
}

fn build_request<T>(payload: T, headers: Vec<(String, String)>) -> Result<Request<T>, ClientError> {
    let mut request = Request::new(payload);
    for (k, v) in headers {
        let key = MetadataKey::from_str(&k).map_err(|source| ClientError::InvalidMetadataKey {
            key: k.clone(),
            source,
        })?;
        let val = MetadataValue::from_str(&v)
            .map_err(|source| ClientError::InvalidMetadataValue { key: k, source })?;
        request.metadata_mut().insert(key, val);
    }
    Ok(request)
}
