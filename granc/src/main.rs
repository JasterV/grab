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
use formatter::{FormattedString, GenericError, ServiceList};
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Call {
            endpoint,
            url,
            body,
            headers,
            file_descriptor_set,
        } => {
            let (service, method) = endpoint;

            let request = DynamicRequest {
                body,
                headers,
                service,
                method,
            };

            let mut client = unwrap_or_exit(GrancClient::connect(&url).await);

            if let Some(path) = file_descriptor_set {
                let fd_bytes = unwrap_or_exit(std::fs::read(&path));
                let mut client = unwrap_or_exit(client.with_file_descriptor(fd_bytes));
                let response = unwrap_or_exit(client.dynamic(request).await);
                print_response(response);
            } else {
                let response = unwrap_or_exit(client.dynamic(request).await);
                print_response(response);
            }
        }

        Commands::List { source } => {
            match source.value() {
                Source::Url(url) => {
                    // Online (Reflection)
                    let mut client = unwrap_or_exit(GrancClient::connect(&url).await);
                    let services = unwrap_or_exit(
                        client
                            .list_services()
                            .await
                            .map_err(|err| GenericError("Failed to list services:", err)),
                    );
                    println!("{}", FormattedString::from(ServiceList(services)));
                }
                Source::File(path) => {
                    // Offline (File)
                    let fd_bytes = unwrap_or_exit(std::fs::read(&path));
                    let client = unwrap_or_exit(GrancClient::offline(fd_bytes));
                    let services = client.list_services();
                    println!("{}", FormattedString::from(ServiceList(services)));
                }
            }
        }

        Commands::Describe { symbol, source } => {
            match source.value() {
                Source::Url(url) => {
                    // Online (Reflection)
                    let mut client = unwrap_or_exit(GrancClient::connect(&url).await);
                    let descriptor = unwrap_or_exit(client.get_descriptor_by_symbol(&symbol).await);
                    print_descriptor(descriptor);
                }
                Source::File(path) => {
                    // Offline (File)
                    let fd_bytes = unwrap_or_exit(std::fs::read(&path));
                    let client = unwrap_or_exit(GrancClient::offline(fd_bytes));
                    let descriptor = unwrap_or_exit(
                        client
                            .get_descriptor_by_symbol(&symbol)
                            .ok_or(GenericError("Symbol not found", symbol)),
                    );
                    print_descriptor(descriptor);
                }
            }
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
