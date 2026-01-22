pub mod client;
mod codec;
pub mod reflection;

pub use client::GrpcClient;

// Re-exports
pub use prost;
pub use prost_reflect;
pub use tonic;

/// Type alias for the standard boxed error used in generic bounds.
type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
