//! # gRPC Client Module
//!
//! This module provides the `GrpcClient`, which is responsible for establishing connections
//! to gRPC servers and performing requests dynamically.
//!
//! Unlike standard gRPC clients that are generated at compile-time, this client uses
//! `prost-reflect` to look up service and method definitions at runtime.

use crate::codec::JsonCodec;
use prost_reflect::DescriptorPool;
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

/// Represents all possible errors that can occur within the gRPC Client.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Invalid uri '{addr}' provided: '{source}'")]
    InvalidUri {
        addr: String,
        source: http::uri::InvalidUri,
    },

    #[error("Failed to connect to '{addr}': '{source}'")]
    ConnectionFailed {
        addr: String,
        source: tonic::transport::Error,
    },

    #[error("File descriptor set decoding failed")]
    InvalidDescriptor(#[from] prost_reflect::DescriptorError),

    #[error("Service '{0}' not found in the provided descriptor set")]
    ServiceNotFound(String),

    #[error("Method '{method}' not found in service '{service}'")]
    MethodNotFound { service: String, method: String },

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

    #[error("Internal error, the client was not ready to send the request: '{0}'")]
    ClientNotReady(#[from] tonic::transport::Error),

    #[error("gRPC execution failed: '{0}'")]
    GrpcStatus(#[from] tonic::Status),
}

/// A dynamic gRPC client wrapper.
///
/// It holds the network channel (connection) and the pool of Protobuf descriptors
/// required to serialize and deserialize messages at runtime.
pub struct GrpcClient {
    channel: Channel,
    pool: DescriptorPool,
}

impl GrpcClient {
    /// Creates a new `GrpcClient`.
    ///
    /// # Arguments
    ///
    /// * `addr` - The server address to connect to (e.g., `http://localhost:50051`).
    /// * `descriptor_bytes` - The raw bytes of the `descriptor.bin` file.
    ///
    /// # Returns
    ///
    /// A connected client instance or a `ClientError`.    
    pub async fn new(addr: &str, descriptor_bytes: &[u8]) -> Result<Self, ClientError> {
        let pool = DescriptorPool::decode(descriptor_bytes)?;

        let channel = Channel::from_shared(addr.to_string())
            .map_err(|source| ClientError::InvalidUri {
                addr: addr.to_string(),
                source,
            })?
            .connect()
            .await
            .map_err(|source| ClientError::ConnectionFailed {
                addr: addr.to_string(),
                source,
            })?;

        Ok(Self { channel, pool })
    }

    /// Performs a unary gRPC call (Request -> Response).
    ///
    /// This method:
    /// 1. Looks up the service and method definitions in the descriptor pool.
    /// 2. Sends the request using a custom `JsonCodec`.
    /// 3. Decodes the response back into JSON.
    ///
    /// # Arguments
    /// * `service_name` - The full name of the service (e.g. `my.package.MyService`)
    /// * `method_name` - The short name of the method (e.g. `MyMethod`)
    /// * `payload` - The JSON request body.
    /// * `headers` - A list of custom headers to attach.
    pub async fn unary(
        &self,
        service_name: &str,
        method_name: &str,
        payload: serde_json::Value,
        headers: Vec<(String, String)>,
    ) -> Result<serde_json::Value, ClientError> {
        let method_name = method_name.trim();
        let service_name = service_name.trim();

        let service_descriptor = self
            .pool
            .get_service_by_name(service_name)
            .ok_or_else(|| ClientError::ServiceNotFound(service_name.to_string()))?;

        let method_descriptor = service_descriptor
            .methods()
            .find(|m| m.name() == method_name)
            .ok_or_else(|| ClientError::MethodNotFound {
                service: service_name.to_string(),
                method: method_name.to_string(),
            })?;

        // Construct URI: "/<ServiceName>/<MethodName>"
        let uri_path = format!(
            "/{}/{}",
            service_descriptor.full_name(),
            method_descriptor.name()
        );

        let path =
            http::uri::PathAndQuery::from_str(&uri_path).expect("gRPC path to always be valid");

        let mut request = Request::new(payload);

        for (k, v) in headers {
            let key =
                MetadataKey::from_str(&k).map_err(|source| ClientError::InvalidMetadataKey {
                    key: k.clone(),
                    source,
                })?;
            let val = MetadataValue::from_str(&v)
                .map_err(|source| ClientError::InvalidMetadataValue { key: k, source })?;

            request.metadata_mut().insert(key, val);
        }

        let codec = JsonCodec::new(method_descriptor.input(), method_descriptor.output());

        let mut client = Grpc::new(self.channel.clone());
        // Ensure the channel is ready to accept a request
        client.ready().await?;

        let response = client.unary(request, path, codec).await?;

        Ok(response.into_inner())
    }
}
