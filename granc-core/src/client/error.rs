use crate::{grpc::client::GrpcRequestError, reflection::client::ReflectionResolveError};
use prost_reflect::DescriptorError;

#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicCallError {
    #[error("Invalid input: '{0}'")]
    InvalidInput(String),

    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    #[error("Method '{0}' not found")]
    MethodNotFound(String),

    #[error("gRPC client request error: '{0}'")]
    GrpcRequestError(#[from] GrpcRequestError),
}

pub mod with_reflection {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum DynamicCallError {
        #[error("Reflection resolution failed: '{0}'")]
        ReflectionResolve(#[from] ReflectionResolveError),

        #[error("Failed to decode file descriptor set: '{0}'")]
        DescriptorError(#[from] DescriptorError),

        #[error(transparent)]
        DynamicCallError(#[from] super::DynamicCallError),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum GetDescriptorError {
        #[error("Reflection resolution failed: '{0}'")]
        ReflectionResolve(#[from] ReflectionResolveError),
        #[error("Failed to decode file descriptor set: '{0}'")]
        DescriptorError(#[from] DescriptorError),
        #[error("Descriptor at path '{0}' not found")]
        NotFound(String),
    }
}
