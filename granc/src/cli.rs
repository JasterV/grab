//! # Command Line Interface Definition
//!
//! This module utilizes `clap` to define the command-line arguments and flags
//! accepted by the application. It acts as the user-facing entry point, responsible for:
//!
//! 1. **Parsing**: extracting and ensuring each argument and flag can be parsed to the target Rust types.
//! 2. **Conversion**: transforming the raw arguments into the `crate::core::Input` struct used by the core logic.
//!
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "granc", version, about = "Dynamic gRPC CLI")]
pub struct Cli {
    #[arg(long, help = "Path to the descriptor set (.bin)")]
    pub proto_set: Option<PathBuf>,

    #[arg(long, help = "JSON body (Object for Unary, Array for Streaming)", value_parser = parse_body)]
    pub body: serde_json::Value,

    #[arg(short = 'H', long = "header", value_parser = parse_header)]
    pub headers: Vec<(String, String)>,

    #[arg(help = "Server URL (http://host:port)")]
    pub url: String,

    #[arg(help = "Endpoint (package.Service/Method)", value_parser = parse_endpoint)]
    pub endpoint: (String, String),
}

impl From<Cli> for crate::core::Input {
    /// Converts the raw CLI arguments into the internal `Input` representation.
    fn from(value: Cli) -> Self {
        let (service, method) = value.endpoint;

        Self {
            proto_set: value.proto_set,
            body: value.body,
            headers: value.headers,
            url: value.url,
            service,
            method,
        }
    }
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
