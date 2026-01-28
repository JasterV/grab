//! # Client State: Online (With Server Reflection)
//!
//! This module defines the `GrancClient` behavior when it is connected to a server
//! and using Server Reflection for schema resolution.
use super::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, Online, OnlineWithoutReflection,
};
use crate::{
    BoxError,
    client::Offline,
    grpc::client::GrpcClient,
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
use http_body::Body as HttpBody;
use prost_reflect::{DescriptorError, DescriptorPool};
use std::fmt::Debug;
use tonic::{
    Code,
    transport::{Channel, Endpoint},
};

/// Errors that can occur when connecting to a gRPC server.
#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URI '{0}': {1}")]
    InvalidUri(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

/// Errors that can occur during a dynamic call in Online mode.
#[derive(Debug, thiserror::Error)]
pub enum DynamicCallError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),
    #[error(transparent)]
    DynamicCallError(#[from] super::online_without_reflection::DynamicCallError),
}

/// Errors that can occur when looking up a descriptor in Online mode.
#[derive(Debug, thiserror::Error)]
pub enum GetDescriptorError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),
    #[error("Descriptor at path '{0}' not found")]
    NotFound(String),
}

impl GrancClient<Online<Channel>> {
    /// Connects to a gRPC server and initializes the client in the `Online` state.
    ///
    /// This is the entry point for interacting with a server. By default, the client assumes
    /// the server supports the gRPC Server Reflection Protocol.
    ///
    /// # Arguments
    ///
    /// * `addr` - The server URI (e.g., `http://localhost:50051`).
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<Online>)` - A connected client ready to make dynamic requests via reflection.
    /// * `Err(ClientConnectError)` - If the URI is invalid or the TCP connection cannot be established.
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUri(addr.to_string(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientConnectError::ConnectionFailed(addr.to_string(), e))?;

        Ok(GrancClient::from(channel))
    }
}

impl<S> From<S> for GrancClient<Online<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    fn from(service: S) -> Self {
        let reflection_client = ReflectionClient::new(service.clone());
        let grpc_client = GrpcClient::new(service);
        Self {
            state: Online {
                reflection_client,
                grpc_client,
            },
        }
    }
}

impl<S> GrancClient<Online<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    /// Transitions the client to the **OnlineWithoutReflection** state by loading a local descriptor.
    ///
    /// This methods consumes the current client and returns a new one that:
    /// 1. Retains the existing network connection.
    /// 2. Disables Server Reflection lookups.
    /// 3. Uses the provided `FileDescriptorSet` for all future schema resolutions.
    ///
    /// This is useful when the server does not support reflection or when you want to enforce
    /// a specific schema version.
    ///
    /// # Arguments
    ///
    /// * `file_descriptor` - A vector of bytes containing the encoded `FileDescriptorSet` (protobuf binary format).
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<OnlineWithoutReflection>)` - The client in the new state.
    /// * `Err(DescriptorError)` - If the provided bytes cannot be decoded into a valid descriptor pool.
    pub fn with_file_descriptor(
        self,
        file_descriptor: Vec<u8>,
    ) -> Result<GrancClient<OnlineWithoutReflection<S>>, DescriptorError> {
        let pool = DescriptorPool::decode(file_descriptor.as_slice())?;

        Ok(GrancClient::new(OnlineWithoutReflection::new(
            self.state.grpc_client,
            pool,
        )))
    }

    /// Lists all services exposed by the server using the Reflection Protocol.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - A list of fully qualified service names (e.g., `helloworld.Greeter`).
    /// * `Err(ReflectionResolveError)` - If the reflection call fails, the stream is closed unexpectedly,
    ///   or the server returns an error code.
    pub async fn list_services(&mut self) -> Result<Vec<String>, ReflectionResolveError> {
        self.state.reflection_client.list_services().await
    }

    /// Resolves and fetches the descriptor for a specific symbol using Reflection.
    ///
    /// This will query the server for the symbol, fetch the defining file, and recursively fetch
    /// all imported dependencies to build a complete `Descriptor`.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The fully qualified name of the symbol (Service, Message, or Enum).
    ///
    /// # Returns
    ///
    /// * `Ok(Descriptor)` - The resolved descriptor wrapper.
    /// * `Err(GetDescriptorError)` - If the symbol is not found on the server or the reflection request fails.
    pub async fn get_descriptor_by_symbol(
        &mut self,
        symbol: &str,
    ) -> Result<Descriptor, GetDescriptorError> {
        let fd_set = self
            .state
            .reflection_client
            .file_descriptor_set_by_symbol(symbol)
            .await
            .map_err(|err| match err {
                ReflectionResolveError::ServerStreamFailure(status)
                    if status.code() == Code::NotFound =>
                {
                    GetDescriptorError::NotFound(symbol.to_string())
                }
                err => GetDescriptorError::ReflectionResolve(err),
            })?;

        let pool = DescriptorPool::from_file_descriptor_set(fd_set)?;
        let client = GrancClient::new(Offline::new(pool));

        client
            .get_descriptor_by_symbol(symbol)
            .ok_or_else(|| GetDescriptorError::NotFound(symbol.to_string()))
    }

    /// Executes a dynamic gRPC request using Server Reflection for schema resolution.
    ///
    /// # Arguments
    ///
    /// * `request` - A [`DynamicRequest`] struct containing:
    ///   - `service`: The fully qualified name of the service (e.g., `my.package.MyService`).
    ///   - `method`: The name of the method to call (e.g., `MyMethod`).
    ///   - `body`: The JSON payload (Object for Unary/ServerStreaming, Array for Client/BiDi Streaming).
    ///   - `headers`: Optional gRPC metadata/headers.
    ///
    /// # Returns
    ///
    /// * `Ok(DynamicResponse)` - The result of the call, which can be:
    ///   - [`DynamicResponse::Unary`]: For Unary and Client Streaming calls (single response).
    ///   - [`DynamicResponse::Streaming`]: For Server Streaming and Bidirectional calls (stream of responses).
    /// * `Err(DynamicCallError)` - If an error occurs during:
    ///   - Reflection resolution (e.g., Service not found).
    ///   - Schema parsing.
    ///   - Request serialization (JSON to Proto).
    ///   - Network transport.
    ///   - Response deserialization.
    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let fd_set = self
            .state
            .reflection_client
            .file_descriptor_set_by_symbol(&request.service)
            .await?;

        let pool = DescriptorPool::from_file_descriptor_set(fd_set)?;

        let mut client = GrancClient::new(OnlineWithoutReflection::new(
            self.state.grpc_client.clone(),
            pool,
        ));

        Ok(client.dynamic(request).await?)
    }
}
