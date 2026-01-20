#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../README.md"))]

/// # gRab CLI Entry Point
///
/// The main module orchestrates the CLI workflow:
/// 1. Parses command-line arguments.
/// 2. Loads the Protobuf descriptor registry.
/// 3. Connects to the gRPC server.
/// 4. Dispatches the request to the appropriate method type (Unary, Streaming, etc.).
use clap::Parser;
use client::GrpcClient;
use descriptor::DescriptorRegistry;
use futures_util::{Stream, StreamExt};
use prost_reflect::MethodDescriptor;
use std::path::PathBuf;
use std::process;

mod client;
mod codec;
mod descriptor;

#[derive(Parser)]
#[command(name = "grab", version, about = "Dynamic gRPC CLI")]
struct Cli {
    #[arg(long, help = "Path to the descriptor set (.bin)")]
    proto_set: PathBuf,

    #[arg(long, help = "JSON body (Object for Unary, Array for Streaming)")]
    body: String,

    #[arg(short = 'H', long = "header", value_parser = parse_header)]
    headers: Vec<(String, String)>,

    #[arg(help = "Server URL (http://host:port)")]
    url: String,

    #[arg(help = "Method (package.Service/Method)")]
    method: String,
}

fn parse_header(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| "Format must be 'key:value'".to_string())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let args = Cli::parse();

    let registry = DescriptorRegistry::from_file(&args.proto_set)?;
    let method = registry.fetch_method_descriptor(&args.method)?;

    let body_json: serde_json::Value =
        serde_json::from_str(&args.body).map_err(|e| anyhow::anyhow!("Invalid JSON: {}", e))?;

    let client = GrpcClient::connect(&args.url).await?;

    println!("Calling {}...", args.method);

    match (method.is_client_streaming(), method.is_server_streaming()) {
        (false, false) => handle_unary(client, method, body_json, args.headers).await,
        (false, true) => handle_server_stream(client, method, body_json, args.headers).await,
        (true, false) => handle_client_stream(client, method, body_json, args.headers).await,
        (true, true) => handle_bidirectional_stream(client, method, body_json, args.headers).await,
    }
}

// --- Handlers ---

async fn handle_unary(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<()> {
    match client.unary(method, body, headers).await? {
        Ok(val) => print_json(&val),
        Err(status) => print_status(status),
    }
    Ok(())
}

async fn handle_server_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<()> {
    match client.server_streaming(method, body, headers).await? {
        Ok(stream) => print_stream(stream).await,
        Err(status) => print_status(status),
    }
    Ok(())
}

async fn handle_client_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let input_stream = json_array_to_stream(body)?;
    match client
        .client_streaming(method, input_stream, headers)
        .await?
    {
        Ok(val) => print_json(&val),
        Err(status) => print_status(status),
    }
    Ok(())
}

async fn handle_bidirectional_stream(
    client: GrpcClient,
    method: MethodDescriptor,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let input_stream = json_array_to_stream(body)?;
    match client
        .bidirectional_streaming(method, input_stream, headers)
        .await?
    {
        Ok(stream) => print_stream(stream).await,
        Err(status) => print_status(status),
    }
    Ok(())
}

// --- Helpers ---

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

fn print_json(val: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string())
    );
}

fn print_status(status: tonic::Status) {
    eprintln!(
        "gRPC Failed: code={:?} message={:?}",
        status.code(),
        status.message()
    );
}

async fn print_stream(
    mut stream: impl Stream<Item = Result<serde_json::Value, tonic::Status>> + Unpin,
) {
    while let Some(result) = stream.next().await {
        match result {
            Ok(val) => print_json(&val),
            Err(status) => print_status(status),
        }
    }
}
