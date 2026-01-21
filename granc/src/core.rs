/// # Granc CLI core implementation
///
/// The core module orchestrates the CLI workflow:
/// 1. Parses command-line arguments.
/// 2. Loads the Protobuf descriptor registry.
/// 3. Connects to the gRPC server.
/// 4. Dispatches the request to the appropriate method type (Unary, Streaming, etc.).
mod client;
mod codec;
mod reflection;

use client::GrpcClient;
use futures_util::{Stream, StreamExt};
use prost_reflect::MethodDescriptor;
use reflection::{DescriptorRegistry, ReflectionClient};
use std::path::PathBuf;

pub struct Input {
    pub proto_set: Option<PathBuf>,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,
    pub url: String,
    pub service: String,
    pub method: String,
}

pub enum Output {
    Unary(Result<serde_json::Value, tonic::Status>),
    Streaming(Result<Vec<Result<serde_json::Value, tonic::Status>>, tonic::Status>),
}

pub async fn run(input: Input) -> anyhow::Result<Output> {
    let registry = match input.proto_set {
        Some(path) => DescriptorRegistry::from_file(path)?,
        // If no proto-set file is passed, we'll try to reach the server reflection service
        None => {
            let mut service = ReflectionClient::connect(input.url.clone()).await?;
            service.get_service_descriptor(&input.service).await?
        }
    };

    let method = registry.get_method_descriptor(&input.service, &input.method)?;

    let client = GrpcClient::connect(&input.url).await?;

    println!("Calling {}/{}...", input.service, input.method);

    match (method.is_client_streaming(), method.is_server_streaming()) {
        (false, false) => handle_unary(client, method, input.body, input.headers).await,
        (false, true) => handle_server_stream(client, method, input.body, input.headers).await,
        (true, false) => handle_client_stream(client, method, input.body, input.headers).await,
        (true, true) => {
            handle_bidirectional_stream(client, method, input.body, input.headers).await
        }
    }
}

// --- Handlers ---

async fn handle_unary(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<Output> {
    let result = client.unary(method, body, headers).await?;
    Ok(Output::Unary(result))
}

async fn handle_server_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<Output> {
    match client.server_streaming(method, body, headers).await? {
        Ok(stream) => Ok(Output::Streaming(Ok(stream.collect().await))),
        Err(status) => Ok(Output::Streaming(Err(status))),
    }
}

async fn handle_client_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<Output> {
    let input_stream = json_array_to_stream(body)?;

    let result = client
        .client_streaming(method, input_stream, headers)
        .await?;

    Ok(Output::Unary(result))
}

async fn handle_bidirectional_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<Output> {
    let input_stream = json_array_to_stream(body)?;

    match client
        .bidirectional_streaming(method, input_stream, headers)
        .await?
    {
        Ok(stream) => Ok(Output::Streaming(Ok(stream.collect().await))),
        Err(status) => Ok(Output::Streaming(Err(status))),
    }
}

fn json_array_to_stream(
    json: serde_json::Value,
) -> anyhow::Result<impl Stream<Item = serde_json::Value> + Send + 'static> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err(anyhow::anyhow!(
            "Client streaming requires a JSON Array body"
        )),
    }
}
