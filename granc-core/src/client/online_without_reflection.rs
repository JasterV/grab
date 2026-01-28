//! # Client State: Online Without Reflection
//!
//! This module defines the `GrancClient` behavior when it is connected to a server
//! but uses a local, in-memory `DescriptorPool` (Static schema) to resolve messages.
use super::{DynamicRequest, DynamicResponse, GrancClient, OnlineWithoutReflection};
use crate::{BoxError, client::OfflineReflectionState, grpc::client::GrpcRequestError};
use futures_util::{Stream, StreamExt};
use http_body::Body as HttpBody;
use std::fmt::Debug;

/// Errors that can occur during a dynamic call in OnlineWithoutReflection mode.
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

impl<S> GrancClient<OnlineWithoutReflection<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    /// Executes a dynamic gRPC request using the locally loaded `FileDescriptorSet`.
    ///
    /// Unlike the `Online` state, this method does **not** make any calls to the server's reflection endpoint.
    /// It relies entirely on the local `DescriptorPool` provided during state transition (see [`super::Online::with_file_descriptor`]).
    ///
    /// # Arguments
    ///
    /// * `request` - A [`DynamicRequest`] struct containing:
    ///   - `service`: The fully qualified name of the service (e.g., `my.package.MyService`).
    ///   - `method`: The name of the method to call (e.g., `MyMethod`).
    ///   - `body`: The JSON payload.
    ///   - `headers`: Optional gRPC metadata.
    ///
    /// # Returns
    ///
    /// * `Ok(DynamicResponse)` - The result of the call (Unary or Streaming).
    /// * `Err(DynamicCallError)` - If validation fails or the network call errors. Specific errors include:
    ///   - [`DynamicCallError::ServiceNotFound`]: The service is not present in the local descriptor.
    ///   - [`DynamicCallError::MethodNotFound`]: The method does not exist in the service.
    ///   - [`DynamicCallError::InvalidInput`]: The JSON body structure is invalid for the streaming mode (e.g. object provided for streaming call).
    ///   - [`DynamicCallError::GrpcRequestError`]: Transport-level errors (connection failed, timeout, etc).
    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let method = self
            .state
            .descriptor_pool()
            .get_service_by_name(&request.service)
            .ok_or_else(|| DynamicCallError::ServiceNotFound(request.service.clone()))?
            .methods()
            .find(|m| m.name() == request.method)
            .ok_or_else(|| DynamicCallError::MethodNotFound(request.method.clone()))?;

        match (method.is_client_streaming(), method.is_server_streaming()) {
            (false, false) => {
                let result = self
                    .state
                    .grpc_client
                    .unary(method, request.body, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }
            (false, true) => match self
                .state
                .grpc_client
                .server_streaming(method, request.body, request.headers)
                .await?
            {
                Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
            },
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

fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, String> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err("Client streaming requires a JSON Array body".to_string()),
    }
}
