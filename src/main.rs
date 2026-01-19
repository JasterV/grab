mod client;
mod codec;

use crate::client::GrpcClient;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "grab",
    version,
    about = "gRab: A dynamic gRPC CLI (gRPC + Crab)",
    long_about = "gRab is a Rust-based CLI tool that allows you to make gRPC calls dynamically using JSON."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Perform a unary gRPC request
    Unary {
        /// Path to the FileDescriptorSet binary
        #[arg(short, long)]
        descriptor: PathBuf,

        /// Server address (e.g., http://[::1]:50051)
        #[arg(short, long)]
        addr: String,

        /// Full service name (e.g., my.package.MyService)
        #[arg(short, long)]
        service: String,

        /// Method name (e.g., CreateUser)
        #[arg(short, long)]
        method: String,

        /// Request JSON body
        #[arg(short, long)]
        json: String,

        /// Headers in "key:value" format
        #[arg(short = 'H', long = "header", value_parser = parse_header)]
        headers: Vec<(String, String)>,
    },
}

fn parse_header(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| "Header format must be 'key:value'".to_string())
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Unary {
            descriptor,
            addr,
            service,
            method,
            json,
            headers,
        } => {
            let json_value: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: Invalid JSON format");
                    eprintln!("  ╰─> {}", e);
                    std::process::exit(1);
                }
            };

            let file_descriptor_set = match std::fs::read(descriptor) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: Initialization failed");
                    eprintln!("  ╰─> {}", e);
                    std::process::exit(1);
                }
            };

            let client = match GrpcClient::new(&addr, &file_descriptor_set).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: Initialization failed");
                    eprintln!("  ╰─> {}", e);
                    std::process::exit(1);
                }
            };

            println!("Connecting to {}...", addr);
            println!("Calling {}/{}", service, method);

            match client.unary(&service, &method, json_value, headers).await {
                Ok(response_val) => match serde_json::to_string_pretty(&response_val) {
                    Ok(pretty) => println!("{}", pretty),
                    Err(_) => println!("{}", response_val),
                },
                Err(e) => {
                    eprintln!("Error: RPC call failed");
                    eprintln!("  ╰─> {}", e);
                    std::process::exit(2);
                }
            }
        }
    }
}
