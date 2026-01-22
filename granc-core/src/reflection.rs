//! # Server Reflection
//!
//! This module contains the logic necessary to interact with the gRPC Server Reflection Protocol.
//!
//! It enables the client to query a server for its own Protobuf schema at runtime, allowing
//! `granc` to function without pre-compiled descriptors.
pub mod client;
mod generated;
