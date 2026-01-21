#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod cli;

use clap::Parser;
use cli::Cli;
use granc::Output;
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    match granc::run(granc::Input::from(args)).await {
        Ok(Output::Unary(Ok(value))) => print_json(&value),
        Ok(Output::Unary(Err(status))) => print_status(&status),
        Ok(Output::Streaming(Ok(values))) => print_stream(&values),
        Ok(Output::Streaming(Err(status))) => print_status(&status),
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
