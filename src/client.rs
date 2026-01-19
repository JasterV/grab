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

    #[error("gRPC execution failed: '{0}'")]
    GrpcStatus(#[from] tonic::Status),
}

pub struct GrpcClient {
    channel: Channel,
    pool: DescriptorPool,
}

impl GrpcClient {
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

    /// Performs a unary gRPC call.
    ///
    /// # Arguments
    /// * `service_name` - The full name of the service (e.g. `my.package.MyService`)
    /// * `method_name` - The short name of the method (e.g. `MyMethod`)
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

        let response = Grpc::new(self.channel.clone())
            .unary(request, path, codec)
            .await?
            .into_inner();

        Ok(response)
    }
}
