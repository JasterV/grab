//! # Granc CLI Entry Point
//!
//! The main executable for the Granc tool. This file drives the application lifecycle:
//!
//! 1. **Initialization**: Parses command-line arguments using [`cli::Cli`].
//! 2. **Connection**: Establishes a TCP connection to the target server via `granc_core`.
//! 3. **Execution**: Delegates the request processing to the `GrancClient`.
//! 4. **Presentation**: Formats and prints the resulting data or errors to standard output/error.

mod cli;
mod formatter;

use clap::Parser;
use cli::{Cli, Commands};
use formatter::FormattedString;
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use std::process;

use crate::formatter::ServiceList;

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    // The URL is now a global argument, available for all commands
    let url = args.url;

    match args.command {
        Commands::Call {
            endpoint,
            body,
            headers,
            file_descriptor_set,
        } => {
            let (service, method) = endpoint;
            run_call(url, service, method, body, headers, file_descriptor_set).await;
        }
        Commands::List => list_services(&url).await,
        Commands::Describe { symbol } => describe_type(&url, &symbol).await,
    }
}

async fn connect_or_exit(url: &str) -> GrancClient {
    match GrancClient::connect(url).await {
        Ok(client) => client,
        Err(err) => {
            eprintln!("{}", FormattedString::from(err));
            process::exit(1);
        }
    }
}

async fn list_services(url: &str) {
    let mut client = connect_or_exit(url).await;

    match client.list_services().await {
        Ok(services) => {
            println!("{}", FormattedString::from(ServiceList(services)));
        }
        Err(e) => {
            eprintln!("{}", FormattedString::from(e));
            process::exit(1);
        }
    }
}

async fn describe_type(url: &str, symbol: &str) {
    let mut client = connect_or_exit(url).await;

    match client.get_descriptor_by_symbol(symbol).await {
        Ok(Descriptor::MessageDescriptor(descriptor)) => {
            println!("{}", FormattedString::from(descriptor))
        }
        Ok(Descriptor::ServiceDescriptor(descriptor)) => {
            println!("{}", FormattedString::from(descriptor))
        }
        Ok(Descriptor::EnumDescriptor(descriptor)) => {
            println!("{}", FormattedString::from(descriptor))
        }
        Err(e) => {
            eprintln!("{}", FormattedString::from(e));
            process::exit(1);
        }
    }
}

async fn run_call(
    url: String,
    service: String,
    method: String,
    body: serde_json::Value,
    headers: Vec<(String, String)>,
    file_descriptor_set: Option<std::path::PathBuf>,
) {
    let file_descriptor_set = match file_descriptor_set.map(std::fs::read).transpose() {
        Ok(fd) => fd,
        Err(err) => {
            eprintln!("{}", FormattedString::from(err));
            process::exit(1);
        }
    };

    let request = DynamicRequest {
        file_descriptor_set,
        body,
        headers,
        service,
        method,
    };

    let mut client = connect_or_exit(&url).await;

    match client.dynamic(request).await {
        Ok(DynamicResponse::Unary(Ok(value))) => println!("{}", FormattedString::from(value)),
        Ok(DynamicResponse::Unary(Err(status))) => println!("{}", FormattedString::from(status)),
        Ok(DynamicResponse::Streaming(Ok(values))) => print_stream(&values),
        Ok(DynamicResponse::Streaming(Err(status))) => {
            println!("{}", FormattedString::from(status))
        }
        Err(err) => {
            eprintln!("{}", FormattedString::from(err));
            process::exit(1);
        }
    }
}

fn print_stream(stream: &[Result<serde_json::Value, tonic::Status>]) {
    for elem in stream {
        match elem {
            Ok(val) => println!("{}", FormattedString::from(val.clone())),
            Err(status) => println!("{}", FormattedString::from(status.clone())),
        }
    }
}
