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
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
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
    /// # Arguments
    ///
    /// * `addr` - The server URI (e.g., `http://localhost:50051`).
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<Online>)` - The connected client.
    /// * `Err(ClientConnectError)` - If the URL is invalid or connection fails.
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUrl(addr.to_string(), e))?;

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
    /// Transitions to the **OnlineWithoutReflection** state.
    ///
    /// This keeps the connection but disables reflection, forcing the use of the
    /// provided `FileDescriptorSet`.
    ///
    /// # Arguments
    /// * `file_descriptor` - The binary descriptor set to use for all future calls.
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

    /// Lists services available on the server via Reflection.
    pub async fn list_services(&mut self) -> Result<Vec<String>, ReflectionResolveError> {
        self.state.reflection_client.list_services().await
    }

    /// Resolves a symbol using Reflection.
    ///
    /// This fetches the schema from the server and returns a `Descriptor`.
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

    /// Executes a dynamic gRPC request using Reflection for schema resolution.
    ///
    /// 1. Fetches the schema for the given service via reflection.
    /// 2. Builds a temporary `OnlineWithoutReflection` client.
    /// 3. Executes the call.
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
