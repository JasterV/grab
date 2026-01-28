//! # Granc CLI Entry Point
//!
//! The main executable for the Granc tool. This file drives the application lifecycle:
//!
//! 1. **Initialization**: Parses command-line arguments using [`cli::Cli`].
//! 2. **Dispatch**: Routes the command to the appropriate handler based on input arguments
//!    (connecting to server vs loading local file).
//! 3. **Execution**: Delegates request processing to `GrancClient`.
//! 4. **Presentation**: Formats and prints data.
mod cli;
mod formatter;

use clap::Parser;
use cli::{Cli, Commands, Source};
use formatter::{FormattedString, GenericError};
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Call {
            endpoint,
            uri,
            body,
            headers,
            file_descriptor_set,
        } => {
            let response = call(endpoint, uri, body, headers, file_descriptor_set).await;
            println!("{}", FormattedString::from(response))
        }

        Commands::List { source } => {
            let services = list(source.value()).await;
            println!(
                "{}",
                FormattedString::from(formatter::ServiceList(services))
            )
        }

        Commands::Describe { symbol, source } => {
            let descriptor = describe(symbol, source.value()).await;
            println!("{}", FormattedString::from(descriptor))
        }
    }
}

async fn call(
    endpoint: (String, String),
    uri: String,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
    file_descriptor_set: Option<std::path::PathBuf>,
) -> DynamicResponse {
    let (service, method) = endpoint;

    let request = DynamicRequest {
        service,
        method,
        body,
        headers,
    };

    let mut client = GrancClient::connect(&uri).await.unwrap_or_exit();

    if let Some(path) = file_descriptor_set {
        let bytes = std::fs::read(path).unwrap_or_exit();
        let mut client = client.with_file_descriptor(bytes).unwrap_or_exit();
        client.dynamic(request).await.unwrap_or_exit()
    } else {
        client.dynamic(request).await.unwrap_or_exit()
    }
}

async fn list(source: Source) -> Vec<String> {
    match source {
        Source::Uri(uri) => {
            let mut client = GrancClient::connect(&uri).await.unwrap_or_exit();
            client
                .list_services()
                .await
                .map_err(|e| GenericError("Failed to list services:", e))
                .unwrap_or_exit()
        }

        Source::File(path) => {
            let fd_bytes = std::fs::read(path).unwrap_or_exit();
            let client = GrancClient::offline(fd_bytes).unwrap_or_exit();
            client.list_services()
        }
    }
}

async fn describe(symbol: String, source: Source) -> Descriptor {
    match source {
        Source::Uri(uri) => {
            let mut client = GrancClient::connect(&uri).await.unwrap_or_exit();
            client
                .get_descriptor_by_symbol(&symbol)
                .await
                .unwrap_or_exit()
        }

        Source::File(path) => {
            let fd_bytes = std::fs::read(path).unwrap_or_exit();
            let client = GrancClient::offline(fd_bytes).unwrap_or_exit();
            client
                .get_descriptor_by_symbol(&symbol)
                .ok_or(GenericError("Symbol not found", symbol))
                .unwrap_or_exit()
        }
    }
}

// Utility trait to standardize the way we handle errors in the program
trait UnwrapOrExit<T, E> {
    fn unwrap_or_exit(self) -> T;
}

impl<T, E> UnwrapOrExit<T, E> for Result<T, E>
where
    E: Into<FormattedString>,
{
    fn unwrap_or_exit(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", Into::<FormattedString>::into(e));
                process::exit(1);
            }
        }
    }
}
