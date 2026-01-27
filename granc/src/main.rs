//! # Granc CLI Entry Point
//!
//! The main executable for the Granc tool. This file drives the application lifecycle:
//!
//! 1. **Initialization**: Parses command-line arguments using [`cli::Cli`].
//! 2. **Connection**: Establishes a TCP connection to the target server via `granc_core`.
//! 3. **Execution**: Delegates the request processing to the `GrancClient` (handling state transitions).
//! 4. **Presentation**: Formats and prints the resulting data or errors to standard output/error.

mod cli;
mod formatter;

use clap::Parser;
use cli::{Cli, Commands};
use formatter::{FormattedString, GenericError, ServiceList};
use granc_core::client::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient, WithFileDescriptor,
    WithServerReflection,
};
use granc_core::tonic::transport::Channel;
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let client = unwrap_or_exit(GrancClient::connect(&args.url).await);

    if let Some(path) = args.file_descriptor_set {
        let bytes = unwrap_or_exit(std::fs::read(&path));
        let client = unwrap_or_exit(client.with_file_descriptor(bytes));
        handle_file_descriptor_mode(client, args.command).await;
    } else {
        handle_reflection_mode(client, args.command).await;
    }
}

async fn handle_reflection_mode(
    mut client: GrancClient<WithServerReflection<Channel>>,
    command: Commands,
) {
    match command {
        Commands::Call {
            endpoint,
            body,
            headers,
        } => {
            let (service, method) = endpoint;
            let request = DynamicRequest {
                body,
                headers,
                service,
                method,
            };

            let response = unwrap_or_exit(client.dynamic(request).await);
            print_response(response);
        }
        Commands::List => {
            let services = unwrap_or_exit(
                client
                    .list_services()
                    .await
                    .map_err(|err| GenericError("Failed to list services:", err)),
            );
            println!("{}", FormattedString::from(ServiceList(services)));
        }
        Commands::Describe { symbol } => {
            let descriptor = unwrap_or_exit(client.get_descriptor_by_symbol(&symbol).await);
            print_descriptor(descriptor);
        }
    }
}

// --- Handler for File Descriptor Mode ---

async fn handle_file_descriptor_mode(
    mut client: GrancClient<WithFileDescriptor<Channel>>,
    command: Commands,
) {
    match command {
        Commands::Call {
            endpoint,
            body,
            headers,
        } => {
            let (service, method) = endpoint;
            let request = DynamicRequest {
                body,
                headers,
                service,
                method,
            };

            let response = unwrap_or_exit(client.dynamic(request).await);
            print_response(response);
        }
        Commands::List => {
            let services = client.list_services();
            println!("{}", FormattedString::from(ServiceList(services)));
        }
        Commands::Describe { symbol } => {
            let descriptor = unwrap_or_exit(
                client
                    .get_descriptor_by_symbol(&symbol)
                    .ok_or(GenericError("Symbol not found", symbol)),
            );
            print_descriptor(descriptor);
        }
    }
}

/// Helper function to return the Ok value or print the error and exit.
fn unwrap_or_exit<T, E>(result: Result<T, E>) -> T
where
    E: Into<FormattedString>,
{
    match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", Into::<FormattedString>::into(e));
            process::exit(1);
        }
    }
}

fn print_descriptor(descriptor: Descriptor) {
    match descriptor {
        Descriptor::MessageDescriptor(d) => println!("{}", FormattedString::from(d)),
        Descriptor::ServiceDescriptor(d) => println!("{}", FormattedString::from(d)),
        Descriptor::EnumDescriptor(d) => println!("{}", FormattedString::from(d)),
    }
}

fn print_response(response: DynamicResponse) {
    match response {
        DynamicResponse::Unary(Ok(value)) => println!("{}", FormattedString::from(value)),
        DynamicResponse::Unary(Err(status)) => println!("{}", FormattedString::from(status)),
        DynamicResponse::Streaming(Ok(values)) => {
            for elem in values {
                match elem {
                    Ok(val) => println!("{}", FormattedString::from(val)),
                    Err(status) => println!("{}", FormattedString::from(status)),
                }
            }
        }
        DynamicResponse::Streaming(Err(status)) => {
            println!("{}", FormattedString::from(status))
        }
    }
}
