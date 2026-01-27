//! # Granc Client
//!
//! This module implements the high-level logic for executing dynamic gRPC requests.
//!
//! The [`GrancClient`] uses a **Typestate Pattern** to ensure safety and correctness regarding
//! how the Protobuf schema is resolved. It has two possible states:
//!
//! 1. **[`WithServerReflection`]**: The default state. The client is connected
//!    to a server and uses the gRPC Server Reflection Protocol (`grpc.reflection.v1`) to discover
//!    services and fetch schemas on the fly.
//! 2. **[`WithFileDescriptor`]**: The client has been provided with a specific
//!    binary `FileDescriptorSet` (e.g., loaded from a `.bin` file). In this state, reflection is
//!    disabled, and all lookups are performed against the provided file.
//!
//! ## Example: State Transition
//!
//! ```rust,no_run
//! use granc_core::client::GrancClient;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect (starts in Reflection state)
//! let mut client_reflection = GrancClient::connect("http://localhost:50051").await?;
//!
//! // The API here is async
//! let services = client_reflection.list_services().await?;
//!
//! // 2Transition to File Descriptor state
//! let bytes = std::fs::read("descriptor.bin")?;
//! let mut client_fd = client_reflection.with_file_descriptor(bytes)?;
//!
//! // Now operations use the local file and are sync
//! let services = client_fd.list_services();
//! # Ok(())
//! # }
//! ```
pub mod with_file_descriptor;
pub mod with_server_reflection;

use crate::{grpc::client::GrpcClient, reflection::client::ReflectionClient};
use prost_reflect::{DescriptorPool, EnumDescriptor, MessageDescriptor, ServiceDescriptor};
use std::fmt::Debug;
use tonic::transport::Channel;

/// The main client for interacting with gRPC servers dynamically.
///
/// The generic parameter `T` represents the current state of the client, determining
/// its capabilities and how it resolves Protobuf schemas.
#[derive(Clone, Debug)]
pub struct GrancClient<T> {
    state: T,
}

/// The state for a client that uses a local `DescriptorPool` for schema resolution.
#[derive(Debug, Clone)]
pub struct WithFileDescriptor<S = Channel> {
    grpc_client: GrpcClient<S>,
    pool: DescriptorPool,
}

/// The state for a client that uses Server Reflection for schema resolution.
#[derive(Debug, Clone)]
pub struct WithServerReflection<S = Channel> {
    reflection_client: ReflectionClient<S>,
    grpc_client: GrpcClient<S>,
}

/// A request object encapsulating all necessary information to perform a dynamic gRPC call.
#[derive(Debug, Clone)]
pub struct DynamicRequest {
    /// The JSON body of the request.
    /// - For Unary/ServerStreaming: An Object `{}`.
    /// - For ClientStreaming/Bidirectional: An Array of Objects `[{}]`.
    pub body: serde_json::Value,
    /// Custom gRPC metadata (headers) to attach to the request.
    pub headers: Vec<(String, String)>,
    /// The fully qualified name of the service (e.g., `my.package.Service`).
    pub service: String,
    /// The name of the method to call (e.g., `SayHello`).
    pub method: String,
}

/// The result of a dynamic gRPC call.
#[derive(Debug, Clone)]
pub enum DynamicResponse {
    /// A single response message (for Unary and Client Streaming calls).
    Unary(Result<serde_json::Value, tonic::Status>),
    /// A stream of response messages (for Server Streaming and Bidirectional calls).
    Streaming(Result<Vec<Result<serde_json::Value, tonic::Status>>, tonic::Status>),
}

/// A generic wrapper for different types of Protobuf descriptors.
///
/// This enum allows the client to return a single type when resolving symbols,
/// regardless of whether the symbol points to a Service, a Message, or an Enum.
#[derive(Debug, Clone)]
pub enum Descriptor {
    MessageDescriptor(MessageDescriptor),
    ServiceDescriptor(ServiceDescriptor),
    EnumDescriptor(EnumDescriptor),
}

impl Descriptor {
    /// Returns the inner [`MessageDescriptor`] if this variant is `MessageDescriptor`.
    pub fn message_descriptor(&self) -> Option<&MessageDescriptor> {
        match self {
            Descriptor::MessageDescriptor(message_descriptor) => Some(message_descriptor),
            _ => None,
        }
    }

    /// Returns the inner [`ServiceDescriptor`] if this variant is `ServiceDescriptor`.
    pub fn service_descriptor(&self) -> Option<&ServiceDescriptor> {
        match self {
            Descriptor::ServiceDescriptor(service_descriptor) => Some(service_descriptor),
            _ => None,
        }
    }

    /// Returns the inner [`EnumDescriptor`] if this variant is `EnumDescriptor`.
    pub fn enum_descriptor(&self) -> Option<&EnumDescriptor> {
        match self {
            Descriptor::EnumDescriptor(enum_descriptor) => Some(enum_descriptor),
            _ => None,
        }
    }
}
