//! # CLI
//!
//! This module defines the command-line interface of `granc` using `clap`.
//!
//! It is responsible for parsing user input and performing validation (e.g., ensuring headers are `key:value`);
use clap::Parser;
use granc_core::client::DynamicRequest;

#[derive(Parser)]
#[command(name = "granc", version, about = "Dynamic gRPC CLI")]
pub struct Cli {
    #[arg(long, help = "Path to the descriptor set (.bin)", value_parser = parse_file_descriptor_set)]
    pub file_descriptor_set: Option<Vec<u8>>,

    #[arg(long, help = "JSON body (Object for Unary, Array for Streaming)", value_parser = parse_body)]
    pub body: serde_json::Value,

    #[arg(short = 'H', long = "header", value_parser = parse_header)]
    pub headers: Vec<(String, String)>,

    #[arg(help = "Server URL (http://host:port)")]
    pub url: String,

    #[arg(help = "Endpoint (package.Service/Method)", value_parser = parse_endpoint)]
    pub endpoint: (String, String),
}

impl From<Cli> for DynamicRequest {
    /// Converts the raw CLI arguments into the internal `Input` representation.
    fn from(value: Cli) -> Self {
        let (service, method) = value.endpoint;

        Self {
            file_descriptor_set: value.file_descriptor_set,
            body: value.body,
            headers: value.headers,
            service,
            method,
        }
    }
}

fn parse_file_descriptor_set(path: &str) -> Result<Vec<u8>, String> {
    let path = path.trim();

    std::fs::read(path)
        .map_err(|err| format!("Failed to read file descriptor set at path '{path}': '{err}'"))
}

fn parse_endpoint(value: &str) -> Result<(String, String), String> {
    let (service, method) = value.split_once('/').ok_or_else(|| {
        format!("Invalid endpoint format: '{value}'. Expected 'package.Service/Method'",)
    })?;

    if service.trim().is_empty() || method.trim().is_empty() {
        return Err("Service and Method names cannot be empty".to_string());
    }

    Ok((service.to_string(), method.to_string()))
}

fn parse_header(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| "Format must be 'key:value'".to_string())
}

fn parse_body(value: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(value).map_err(|e| format!("Invalid JSON: {e}"))
}
