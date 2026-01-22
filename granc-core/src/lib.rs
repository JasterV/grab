//! # Granc Core
//!
//! `granc-core` is the foundational library powering the Granc CLI. It provides a dynamic
//! gRPC client capable of interacting with any gRPC server without compile-time knowledge
//! of the Protobuf schema.
//!
//! ## Key Components
//!
//! * **[`GrancClient`]:** The main entry point. It orchestrates schema resolution (via reflection
//!   or file descriptors) and dispatches requests to the generic gRPC transport.
//! * **[`DynamicRequest`] & [`DynamicResponse`]:** The primary data structures for I/O, allowing
//!   callers to pass JSON data and receive JSON results.
//!
//! ## Internal clients
//!
//! We've decided to expose the core clients that we use internally to perform gRPC requests using JSON
//! and to interact with a server reflection service.
//!
//! * **[`GrpcClient`]:** A fully-featured dynamic gRPC client using a custom Json Codec.
//! * **[`ReflectionClient`]:** A gRPC Reflection client offering for now only the functionality that we need internally,
//!   might be extended in the future and packaged as a separate crate if the community finds it useful.
//!
//! ## JsonCodec
//!
//! An implementation of `tonic::codec::Codec` that transcodes JSON to Protobuf bytes (and vice versa) on the fly.
//!
// * **Encoder**: Validates `serde_json::Value` against the input `MessageDescriptor` and serializes it.
// * **Decoder**: Deserializes bytes into a `DynamicMessage` and converts it back to `serde_json::Value`.
//!
//! ## Feature Flags (Internal use only)
//!
//! * `gen-proto`: Enables support for generating reflection service bindings (internal use).
//!
//! ## Re-exports
//!
//! This crate re-exports `prost`, `prost-reflect`, and `tonic` to ensure that consumers
//! use compatible versions of these underlying dependencies.
//!
//! See the README.md for more details about usage.
pub mod client;
pub mod grpc;
pub mod reflection;

// Re-exports
pub use prost;
pub use prost_reflect;
pub use tonic;

/// Type alias for the standard boxed error used in generic bounds.
type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
