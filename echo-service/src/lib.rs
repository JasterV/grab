//! # Echo Service
//!
//! **INTERNAL USE ONLY**: This crate exists solely to provide a gRPC server implementation
//! and descriptor set for integration testing the `grab` CLI tool.
//! It is not intended for production use.

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/echo.rs"));
}

pub use pb::echo_service_server::{EchoService, EchoServiceServer};
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptors");
