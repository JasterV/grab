#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod cli;

use clap::Parser;
use cli::Cli;
use granc_core::{Granc, GrpcRequest, GrpcResponse};
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let client = match Granc::connect(&args.url).await {
        Ok(client) => client,
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    };

    match client.call(GrpcRequest::from(args)).await {
        Ok(GrpcResponse::Unary(Ok(value))) => print_json(&value),
        Ok(GrpcResponse::Unary(Err(status))) => print_status(&status),
        Ok(GrpcResponse::Streaming(Ok(values))) => print_stream(&values),
        Ok(GrpcResponse::Streaming(Err(status))) => print_status(&status),
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    }
}

fn print_json(val: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string())
    );
}

fn print_status(status: &tonic::Status) {
    eprintln!(
        "gRPC Failed: code={:?} message={:?}",
        status.code(),
        status.message()
    );
}

fn print_stream(stream: &[Result<serde_json::Value, tonic::Status>]) {
    for elem in stream {
        match elem {
            Ok(val) => print_json(val),
            Err(status) => print_status(status),
        }
    }
}
