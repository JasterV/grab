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
mod types;

pub use types::*;

use crate::{grpc::client::GrpcClient, reflection::client::ReflectionClient};
use prost_reflect::DescriptorPool;
use std::fmt::Debug;
use tonic::transport::Channel;

/// The main client for interacting with gRPC servers dynamically.
///
/// The generic parameter `T` represents the current state of the client.
#[derive(Clone, Debug)]
pub struct GrancClient<T> {
    state: T,
}

impl<T> GrancClient<T> {
    pub(crate) fn new(state: T) -> Self {
        Self { state }
    }
}

/// State: Connected to server, Schema resolved from Server Reflection.
#[derive(Debug, Clone)]
pub struct Online<S = Channel> {
    reflection_client: ReflectionClient<S>,
    grpc_client: GrpcClient<S>,
}

/// State: Connected to server, Schema resolved from local FileDescriptor.
#[derive(Debug, Clone)]
pub struct OnlineWithoutReflection<S = Channel> {
    grpc_client: GrpcClient<S>,
    pool: DescriptorPool,
}

impl<S> OnlineWithoutReflection<S> {
    pub(crate) fn new(grpc_client: GrpcClient<S>, pool: DescriptorPool) -> Self {
        Self { pool, grpc_client }
    }
}

/// State: Disconnected, Schema resolved from local FileDescriptor.
#[derive(Debug, Clone)]
pub struct Offline {
    pool: DescriptorPool,
}

impl Offline {
    pub(crate) fn new(pool: DescriptorPool) -> Self {
        Self { pool }
    }
}

pub trait OfflineReflectionState {
    fn descriptor_pool(&self) -> &DescriptorPool;
}

impl OfflineReflectionState for Offline {
    fn descriptor_pool(&self) -> &DescriptorPool {
        &self.pool
    }
}

impl<S> OfflineReflectionState for OnlineWithoutReflection<S> {
    fn descriptor_pool(&self) -> &DescriptorPool {
        &self.pool
    }
}
