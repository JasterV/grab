//! # Granc Client
//!
//! This module implements the high-level logic for executing dynamic gRPC requests.
//!
//! The [`GrancClient`] uses a **Typestate Pattern** to ensure safety and correctness regarding
//! how the Protobuf schema is resolved. It has three possible states:
//!
//! 1. **[`Online`]**: The default state when connecting. The client uses the gRPC
//!    Server Reflection Protocol (`grpc.reflection.v1`) to discover services.
//! 2. **[`OnlineWithoutReflection`]**: The client is connected to a server but uses a local
//!    binary `FileDescriptorSet` for schema lookups.
//! 3. **[`Offline`]**: The client is **not connected** to any server. It holds a
//!    local `FileDescriptorSet` and can only be used for introspection (Listing services, describing symbols),
//!    but cannot perform gRPC calls.
//!
//! ## Example: State Transition
//!
//! ```rust,no_run
//! use granc_core::client::GrancClient;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Online State (Reflection)
//! let mut client = GrancClient::connect("http://localhost:50051").await?;
//!
//! // 2. Transition to OnlineWithoutReflection (Connected + Local Schema)
//! let bytes = std::fs::read("descriptor.bin")?;
//! let mut client_static = client.with_file_descriptor(bytes)?;
//!
//! // 3. Offline State (Disconnected + Local Schema)
//! let bytes = std::fs::read("descriptor.bin")?;
//! let mut client_offline = GrancClient::offline(bytes)?;
//! # Ok(())
//! # }
//! ```
pub mod offline;
pub mod online;
pub mod online_without_reflection;

use crate::{grpc::client::GrpcClient, reflection::client::ReflectionClient};
use prost_reflect::{DescriptorPool, EnumDescriptor, MessageDescriptor, ServiceDescriptor};
use std::fmt::Debug;
use tonic::transport::Channel;

/// The main client for interacting with gRPC servers dynamically.
///
/// The generic parameter `T` represents the current state of the client.
#[derive(Clone, Debug)]
pub struct GrancClient<T> {
    state: T,
}

/// State: Connected to server, Schema from Server Reflection.
#[derive(Debug, Clone)]
pub struct Online<S = Channel> {
    reflection_client: ReflectionClient<S>,
    grpc_client: GrpcClient<S>,
}

/// State: Connected to server, Schema from local FileDescriptor.
#[derive(Debug, Clone)]
pub struct OnlineWithoutReflection<S = Channel> {
    grpc_client: GrpcClient<S>,
    pool: DescriptorPool,
}

/// State: Disconnected, Schema from local FileDescriptor.
#[derive(Debug, Clone)]
pub struct Offline {
    pool: DescriptorPool,
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
            Descriptor::MessageDescriptor(d) => Some(d),
            _ => None,
        }
    }

    /// Returns the inner [`ServiceDescriptor`] if this variant is `ServiceDescriptor`.
    pub fn service_descriptor(&self) -> Option<&ServiceDescriptor> {
        match self {
            Descriptor::ServiceDescriptor(d) => Some(d),
            _ => None,
        }
    }

    /// Returns the inner [`EnumDescriptor`] if this variant is `EnumDescriptor`.
    pub fn enum_descriptor(&self) -> Option<&EnumDescriptor> {
        match self {
            Descriptor::EnumDescriptor(d) => Some(d),
            _ => None,
        }
    }
}
