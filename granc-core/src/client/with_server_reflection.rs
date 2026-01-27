//! # Client State: Server Reflection
//!
//! This module defines the `GrancClient` behavior when it is using the server's reflection service
//! to resolve schemas.
use super::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, WithFileDescriptor,
    WithServerReflection,
};
use crate::{
    BoxError,
    grpc::client::GrpcClient,
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
use http_body::Body as HttpBody;
use prost_reflect::DescriptorError;
use prost_reflect::DescriptorPool;
use std::fmt::Debug;
use tonic::{
    Code,
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
pub enum DynamicCallError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),

    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),

    #[error(transparent)]
    DynamicCallError(#[from] super::with_file_descriptor::DynamicCallError),
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

impl GrancClient<WithServerReflection<Channel>> {
    /// Connects to a gRPC server at the specified address.
    ///
    /// This initializes the client in the **Reflection** state. It establishes a TCP connection
    /// but does not yet perform any reflection calls.
    ///
    /// # Arguments
    ///
    /// * `addr` - The URI of the server (e.g., `http://localhost:50051`).
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<WithServerReflection>)` - The connected client ready to use reflection.
    /// * `Err(ClientConnectError)` - If the URL is invalid or the connection cannot be established.
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUrl(addr.to_string(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientConnectError::ConnectionFailed(addr.to_string(), e))?;

        Ok(Self::from_service(channel))
    }
}

impl<S> GrancClient<WithServerReflection<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    /// Creates a new `GrancClient` wrapping an existing Tonic service (e.g., a `Channel` or `InterceptedService`).
    ///
    /// # Arguments
    ///
    /// * `service` - The generic gRPC service implementation to use for transport.
    pub fn from_service(service: S) -> Self {
        let reflection_client = ReflectionClient::new(service.clone());
        let grpc_client = GrpcClient::new(service);

        Self {
            state: WithServerReflection {
                reflection_client,
                grpc_client,
            },
        }
    }

    /// Transitions the client to the **File Descriptor** state.
    ///
    /// This method consumes the current client and returns a new client that uses the provided
    /// binary `FileDescriptorSet` for all schema lookups, disabling server reflection.
    ///
    /// # Arguments
    ///
    /// * `file_descriptor` - A vector of bytes containing the encoded `FileDescriptorSet` (protobuf binary format).
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<WithFileDescriptor>)` - The new client state.
    /// * `Err(DescriptorError)` - If the provided bytes could not be decoded into a valid `DescriptorPool`.
    pub fn with_file_descriptor(
        self,
        file_descriptor: Vec<u8>,
    ) -> Result<GrancClient<WithFileDescriptor<S>>, DescriptorError> {
        let pool = DescriptorPool::decode(file_descriptor.as_slice())?;
        Ok(GrancClient::new(self.state.grpc_client, pool))
    }

    /// Lists all services exposed by the server by querying the reflection endpoint.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - A list of fully qualified service names (e.g. `helloworld.Greeter`).
    /// * `Err(ReflectionResolveError)` - If the reflection call fails or the server doesn't support reflection.
    pub async fn list_services(&mut self) -> Result<Vec<String>, ReflectionResolveError> {
        self.state.reflection_client.list_services().await
    }

    /// Resolves and fetches the descriptor for a specific symbol using reflection.
    ///
    /// This will recursively fetch the file defining the symbol and all its dependencies
    /// from the server.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The fully qualified name of the symbol (Service, Message, or Enum).
    ///
    /// # Returns
    ///
    /// * `Ok(Descriptor)` - The resolved descriptor wrapper.
    /// * `Err(GetDescriptorError)` - If the symbol is not found or reflection fails.
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

        let mut client =
            GrancClient::<WithFileDescriptor<S>>::new(self.state.grpc_client.clone(), pool);

        client
            .get_descriptor_by_symbol(symbol)
            .ok_or_else(|| GetDescriptorError::NotFound(symbol.to_string()))
    }

    /// Executes a dynamic gRPC request using reflection.
    ///
    /// 1. It fetches the schema for the requested `service` via reflection.
    /// 2. It builds a temporary `WithFileDescriptor` client using that schema.
    /// 3. It delegates the call to that client.
    ///
    /// # Arguments
    ///
    /// * `request` - The [`DynamicRequest`] containing the method to call and the JSON body.
    ///
    /// # Returns
    ///
    /// * `Ok(DynamicResponse)` - The result of the gRPC call (Unary or Streaming).
    /// * `Err(DynamicCallError)` - If schema resolution, validation, or the network call fails.
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

        let mut client =
            GrancClient::<WithFileDescriptor<S>>::new(self.state.grpc_client.clone(), pool);

        Ok(client.dynamic(request).await?)
    }
}
