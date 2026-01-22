//! # Generic gRPC Transport
//!
//! This module contains the low-level building blocks for performing gRPC calls using
//! dynamic message types.
//!
//! Unlike standard `tonic` clients which are strongly typed (e.g., `HelloRequest`),
//! the components here are designed to work with generic `serde_json::Value` structures,
//! transcoding them to Protobuf binary format on the fly.
pub mod client;
pub mod codec;
