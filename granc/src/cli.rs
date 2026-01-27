//! # CLI
//!
//! This module defines the command-line interface of `granc` using `clap`.
//!
//! It is responsible for parsing user input and performing validation (e.g., ensuring headers are `key:value`);
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "granc", version, about = "Dynamic gRPC CLI")]
pub struct Cli {
    /// The server URL to connect to (e.g. http://localhost:50051)
    pub url: String,

    /// Path to the descriptor set (.bin)
    #[arg(long)]
    pub file_descriptor_set: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Perform a gRPC call to a server
    ///
    /// This command connects to a gRPC server and executes a method using a JSON body.
    Call {
        /// Endpoint (package.Service/Method)
        #[arg(value_parser = parse_endpoint)]
        endpoint: (String, String),
        /// "JSON body (Object for Unary, Array for Streaming)"
        #[arg(long, value_parser = parse_body)]
        body: serde_json::Value,

        #[arg(short = 'H', long = "header", value_parser = parse_header)]
        headers: Vec<(String, String)>,
    },

    /// List available services
    List,

    /// Describe a service, message or enum passing the full path
    Describe {
        /// Fully qualified name (e.g. my.package.Service)
        symbol: String,
    },
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
