//! # Client State: File Descriptor
//!
//! This module defines the `GrancClient` behavior when it is using a local, in-memory
//! `DescriptorPool` (loaded from a file) to resolve schemas.
//!
//! In this state, the client does **not** use server reflection for schema lookup.
use super::WithFileDescriptor;
use super::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use crate::{
    BoxError,
    grpc::client::{GrpcClient, GrpcRequestError},
};
use futures_util::Stream;
use futures_util::StreamExt;
use http_body::Body as HttpBody;
use prost_reflect::DescriptorPool;
use std::fmt::Debug;

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

impl<S> GrancClient<WithFileDescriptor<S>>
where
    S: Clone,
{
    pub(crate) fn new(grpc_client: GrpcClient<S>, pool: DescriptorPool) -> Self {
        Self {
            state: WithFileDescriptor { grpc_client, pool },
        }
    }
}

impl<S> GrancClient<WithFileDescriptor<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    /// Lists all services defined in the loaded `DescriptorPool`.
    ///
    /// Unlike the reflection client, this is a synchronous operation that returns
    /// immediately from memory.
    ///
    /// # Returns
    ///
    /// A list of fully qualified service names (e.g. `helloworld.Greeter`).
    pub fn list_services(&mut self) -> Vec<String> {
        self.state
            .pool
            .services()
            .map(|s| s.full_name().to_string())
            .collect()
    }

    /// Looks up a specific symbol in the loaded `DescriptorPool`.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The fully qualified name of the symbol (Service, Message, or Enum).
    ///
    /// # Returns
    ///
    /// * `Some(Descriptor)` - The resolved descriptor if found.
    /// * `None` - If the symbol does not exist in the pool.
    pub fn get_descriptor_by_symbol(&mut self, symbol: &str) -> Option<Descriptor> {
        let pool = &self.state.pool;

        if let Some(descriptor) = pool.get_service_by_name(symbol) {
            return Some(Descriptor::ServiceDescriptor(descriptor));
        }

        if let Some(descriptor) = pool.get_message_by_name(symbol) {
            return Some(Descriptor::MessageDescriptor(descriptor));
        }

        if let Some(descriptor) = pool.get_enum_by_name(symbol) {
            return Some(Descriptor::EnumDescriptor(descriptor));
        }

        None
    }

    /// Executes a dynamic gRPC request using the loaded `DescriptorPool`.
    ///
    /// It looks up the service and method definitions in the local pool, validates the JSON, and sends the request.
    ///
    /// # Arguments
    ///
    /// * `request` - The [`DynamicRequest`] containing the method to call and the JSON body.
    ///
    /// # Returns
    ///
    /// * `Ok(DynamicResponse)` - The result of the gRPC call.
    /// * `Err(DynamicCallError)` - If the service/method is not in the pool, the JSON is invalid, or the call fails.
    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let method = self
            .state
            .pool
            .get_service_by_name(&request.service)
            .ok_or_else(|| DynamicCallError::ServiceNotFound(request.service))?
            .methods()
            .find(|m| m.name() == request.method)
            .ok_or_else(|| DynamicCallError::MethodNotFound(request.method))?;

        match (method.is_client_streaming(), method.is_server_streaming()) {
            (false, false) => {
                let result = self
                    .state
                    .grpc_client
                    .unary(method, request.body, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (false, true) => {
                match self
                    .state
                    .grpc_client
                    .server_streaming(method, request.body, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
            (true, false) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                let result = self
                    .state
                    .grpc_client
                    .client_streaming(method, input_stream, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (true, true) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                match self
                    .state
                    .grpc_client
                    .bidirectional_streaming(method, input_stream, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
        }
    }
}

/// Helper to convert a JSON Array into a Stream of JSON Values.
/// Required for Client and Bidirectional streaming.
fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, String> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err("Client streaming requires a JSON Array body".to_string()),
    }
}
