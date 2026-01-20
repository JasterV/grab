//! # Descriptor Registry
//!
//! This module handles the loading and querying of Protobuf `FileDescriptorSet`s.
//! It acts as a database of schema definitions, allowing the application to
//! resolve service and method names into `MethodDescriptor` objects required
//! for reflection.

use prost_reflect::{DescriptorPool, MethodDescriptor};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DescriptorError {
    #[error("Failed to read descriptor file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to decode descriptor set: {0}")]
    Decode(#[from] prost_reflect::DescriptorError),
    #[error("Service '{0}' not found")]
    ServiceNotFound(String),
    #[error("Method '{0}' not found")]
    MethodNotFound(String),
    #[error("Invalid method path. Expected format 'package.Service/Method', got '{0}'")]
    InvalidFormat(String),
}

/// A registry that holds loaded Protobuf definitions and allows looking up
/// services and methods by name.
pub struct DescriptorRegistry {
    pool: DescriptorPool,
}

impl DescriptorRegistry {
    /// Decodes a FileDescriptorSet directly from a byte slice.
    /// Useful for tests or embedded descriptors.
    #[cfg(test)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DescriptorError> {
        let pool = DescriptorPool::decode(bytes)?;
        Ok(Self { pool })
    }

    /// Loads a FileDescriptorSet from a file on disk and builds the registry.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, DescriptorError> {
        let bytes = std::fs::read(path)?;
        let pool = DescriptorPool::decode(bytes.as_slice())?;
        Ok(Self { pool })
    }

    /// Resolves a full method path (e.g., "my.package.MyService/MyMethod")
    /// into a MethodDescriptor.
    pub fn fetch_method_descriptor(
        &self,
        method_path: &str,
    ) -> Result<MethodDescriptor, DescriptorError> {
        let (service_name, method_name) = method_path
            .split_once('/')
            .ok_or_else(|| DescriptorError::InvalidFormat(method_path.to_string()))?;

        let service = self
            .pool
            .get_service_by_name(service_name)
            .ok_or_else(|| DescriptorError::ServiceNotFound(service_name.to_string()))?;

        service
            .methods()
            .find(|m| m.name() == method_name)
            .ok_or_else(|| DescriptorError::MethodNotFound(method_name.to_string()))
    }
}
