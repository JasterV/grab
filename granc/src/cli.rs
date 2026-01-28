//! # CLI
//!
//! This module defines the command-line interface of `granc` using `clap`.
//! It enforces strict invariants for arguments using subcommands and argument groups.
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "granc", version, about = "Dynamic gRPC CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Perform a gRPC call to a server.
    ///
    /// Requires a server URI. Can optionally use a local file descriptor set.
    Call {
        /// Endpoint (package.Service/Method)
        #[arg(value_parser = parse_endpoint)]
        endpoint: (String, String),

        /// The server URI to connect to (e.g. http://localhost:50051)
        #[arg(long, short = 'u')]
        uri: String,

        /// "JSON body (Object for Unary, Array for Streaming)"
        #[arg(long, short = 'b', value_parser = parse_body)]
        body: serde_json::Value,

        #[arg(short = 'H', long = "header", value_parser = parse_header)]
        headers: Vec<(String, String)>,

        /// Optional path to a file descriptor set (.bin) to use instead of reflection
        #[arg(long, short = 'f')]
        file_descriptor_set: Option<PathBuf>,
    },

    /// List available services.
    ///
    /// Requires EITHER a server URI (Reflection) OR a file descriptor set (Offline).
    List {
        #[command(flatten)]
        source: SourceSelection,
    },

    /// Describe a service, message or enum.
    ///
    /// Requires EITHER a server URI (Reflection) OR a file descriptor set (Offline).
    Describe {
        #[command(flatten)]
        source: SourceSelection,

        /// Fully qualified name (e.g. my.package.Service)
        symbol: String,
    },
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)] // Enforces: Either URI OR FileDescriptorSet, never both.
pub struct SourceSelection {
    /// The server URI to use for reflection-based introspection
    #[arg(long, short = 'u')]
    uri: Option<String>,

    /// Path to the descriptor set (.bin) to use for offline introspection
    #[arg(long, short = 'f')]
    file_descriptor_set: Option<PathBuf>,
}

// The source where to resolve the proto schemas from.
//
// It can either be a URI (If the server supports server streaming)
// or a file (a `.bin` or `.pb` file generated with protoc)
pub enum Source {
    Uri(String),
    File(PathBuf),
}

impl SourceSelection {
    pub fn value(self) -> Source {
        if let Some(uri) = self.uri {
            Source::Uri(uri)
        } else if let Some(path) = self.file_descriptor_set {
            Source::File(path)
        } else {
            // This is unreachable because `clap` verifies the group requirements before we ever get here.
            unreachable!(
                "Clap ensures exactly one argument (uri or file) is present via #[group(required = true)]"
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_call_command_reflection() {
        let args = vec![
            "granc",
            "call",
            "helloworld.Greeter/SayHello",
            "--uri",
            "http://localhost:50051",
            "--body",
            r#"{"name": "Ferris"}"#,
        ];

        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::Call {
                endpoint,
                uri,
                body,
                file_descriptor_set,
                ..
            } => {
                assert_eq!(
                    endpoint,
                    ("helloworld.Greeter".to_string(), "SayHello".to_string())
                );
                assert_eq!(uri, "http://localhost:50051");
                assert_eq!(body, serde_json::json!({"name": "Ferris"}));
                assert!(file_descriptor_set.is_none());
            }
            _ => panic!("Expected Call command"),
        }
    }

    #[test]
    fn test_call_command_with_file_descriptor() {
        let args = vec![
            "granc",
            "call",
            "helloworld.Greeter/SayHello",
            "--uri",
            "http://localhost:50051",
            "--body",
            r#"{"name": "Ferris"}"#,
            "--file-descriptor-set",
            "./descriptors.bin",
        ];

        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::Call {
                file_descriptor_set,
                ..
            } => {
                assert_eq!(
                    file_descriptor_set.unwrap().to_str().unwrap(),
                    "./descriptors.bin"
                );
            }
            _ => panic!("Expected Call command"),
        }
    }

    #[test]
    fn test_call_command_short_flags() {
        let args = vec![
            "granc",
            "call",
            "svc/mthd",
            "-u",
            "http://localhost:50051",
            "-b",
            "{}",
            "-f",
            "desc.bin",
            "-H",
            "auth:bearer",
        ];

        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::Call {
                uri,
                file_descriptor_set,
                headers,
                body,
                ..
            } => {
                assert_eq!(uri, "http://localhost:50051");
                assert_eq!(file_descriptor_set.unwrap().to_str().unwrap(), "desc.bin");
                assert_eq!(body, serde_json::json!({}));
                assert_eq!(headers[0], ("auth".to_string(), "bearer".to_string()));
            }
            _ => panic!("Expected Call command"),
        }
    }

    #[test]
    fn test_list_command_reflection() {
        let args = vec!["granc", "list", "--uri", "http://localhost:50051"];
        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::List { source } => {
                assert_eq!(source.uri.unwrap(), "http://localhost:50051");
                assert!(source.file_descriptor_set.is_none());
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_list_command_offline() {
        let args = vec!["granc", "list", "--file-descriptor-set", "desc.bin"];
        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::List { source } => {
                assert_eq!(
                    source.file_descriptor_set.unwrap().to_str().unwrap(),
                    "desc.bin"
                );
                assert!(source.uri.is_none());
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_describe_command() {
        let args = vec![
            "granc",
            "describe",
            "helloworld.Greeter",
            "--uri",
            "http://localhost:50051",
        ];
        let cli = Cli::try_parse_from(&args).expect("Parsing failed");

        match cli.command {
            Commands::Describe { symbol, source } => {
                assert_eq!(symbol, "helloworld.Greeter");
                assert!(source.uri.is_some());
            }
            _ => panic!("Expected Describe command"),
        }
    }

    // --- Failure Cases ---

    #[test]
    fn test_fail_invalid_json_body() {
        let args = vec!["granc", "call", "s/m", "-u", "x", "--body", "{invalid_json"];
        let err = Cli::try_parse_from(&args).unwrap_err();
        // Should verify that the error comes from the body parser
        assert!(err.to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_fail_invalid_endpoint_format() {
        let args = vec![
            "granc",
            "call",
            "OnlyServiceNoMethod", // Missing '/'
            "-u",
            "x",
            "-b",
            "{}",
        ];
        let err = Cli::try_parse_from(&args).unwrap_err();
        assert!(err.to_string().contains("Invalid endpoint format"));
    }

    #[test]
    fn test_fail_list_requires_source() {
        let args = vec!["granc", "list"];
        let err = Cli::try_parse_from(&args).unwrap_err();
        // Clap error for missing required arguments in group
        assert!(err.kind() == clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_fail_list_mutual_exclusion() {
        let args = vec![
            "granc",
            "list",
            "--uri",
            "http://host",
            "--file-descriptor-set",
            "file.bin",
        ];
        let err = Cli::try_parse_from(&args).unwrap_err();
        // Clap error for argument conflict
        assert!(err.kind() == clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_fail_describe_mutual_exclusion() {
        let args = vec![
            "granc",
            "describe",
            "Symbol",
            "-u",
            "http://host",
            "-f",
            "file.bin",
        ];
        let err = Cli::try_parse_from(&args).unwrap_err();
        assert!(err.kind() == clap::error::ErrorKind::ArgumentConflict);
    }
}
