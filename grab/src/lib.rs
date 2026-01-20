pub mod client;
pub mod codec;
pub mod descriptor;

// Re-export specific items for easier access
pub use client::GrpcClient;
pub use descriptor::DescriptorRegistry;
