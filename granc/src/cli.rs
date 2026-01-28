//! # CLI
//!
//! This module defines the command-line interface of `granc` using `clap`.
//! It enforces strict invariants for arguments using subcommands and argument groups.
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "granc", version, about = "Dynamic gRPC CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Perform a gRPC call to a server.
    ///
    /// Requires a server URL. Can optionally use a local file descriptor set.
    Call {
        /// Endpoint (package.Service/Method)
        #[arg(value_parser = parse_endpoint)]
        endpoint: (String, String),

        /// The server URL to connect to (e.g. http://localhost:50051)
        #[arg(long, short = 'u', value_parser = parse_body)]
        url: String,

        /// "JSON body (Object for Unary, Array for Streaming)"
        #[arg(long, short = 'b', value_parser = parse_body)]
        body: serde_json::Value,

        #[arg(short = 'h', long = "header", value_parser = parse_header)]
        headers: Vec<(String, String)>,

        /// Optional path to a file descriptor set (.bin) to use instead of reflection
        #[arg(long, short = 'f')]
        file_descriptor_set: Option<PathBuf>,
    },

    /// List available services.
    ///
    /// Requires EITHER a server URL (Reflection) OR a file descriptor set (Offline).
    List {
        #[command(flatten)]
        source: SourceSelection,
    },

    /// Describe a service, message or enum.
    ///
    /// Requires EITHER a server URL (Reflection) OR a file descriptor set (Offline).
    Describe {
        #[command(flatten)]
        source: SourceSelection,

        /// Fully qualified name (e.g. my.package.Service)
        symbol: String,
    },
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)] // Enforces: Either URL OR FileDescriptorSet, never both.
pub struct SourceSelection {
    /// The server URL to use for reflection-based introspection
    #[arg(long, short = 'u')]
    url: Option<String>,

    /// Path to the descriptor set (.bin) to use for offline introspection
    #[arg(long, short = 'f')]
    file_descriptor_set: Option<PathBuf>,
}

// The source where to resolve the proto schemas from.
//
// It can either be a URL (If the server supports server streaming)
// or a file (a `.bin` or `.pb` file generated with protoc)
pub enum Source {
    Url(String),
    File(PathBuf),
}

impl SourceSelection {
    pub fn value(self) -> Source {
        if let Some(url) = self.url {
            Source::Url(url)
        } else if let Some(path) = self.file_descriptor_set {
            Source::File(path)
        } else {
            // This is unreachable because `clap` verifies the group requirements before we ever get here.
            unreachable!(
                "Clap ensures exactly one argument (url or file) is present via #[group(required = true)]"
            )
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
