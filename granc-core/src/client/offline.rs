//! # Client State: Offline
//!
//! This module defines the `GrancClient` behavior when it is using a local, in-memory
//! `DescriptorPool` but is **not connected** to any gRPC server.
//!
//! In this state, the client is strictly limited to introspection tasks.
use super::{Descriptor, GrancClient, Offline};
use prost_reflect::{DescriptorError, DescriptorPool};

impl GrancClient<Offline> {
    /// Creates a new `GrancClient` in the Offline state using a raw byte buffer
    /// containing a `FileDescriptorSet`.
    ///
    /// This client starts in a **disconnected** state. It can be used to inspect the
    /// provided schema but cannot make network requests.
    ///
    /// # Arguments
    ///
    /// * `file_descriptor` - A vector of bytes containing the encoded `FileDescriptorSet`.
    ///
    /// # Returns
    ///
    /// * `Ok(GrancClient<Offline>)` - The initialized offline client.
    /// * `Err(DescriptorError)` - If the bytes are not a valid descriptor set.
    pub fn offline(file_descriptor: Vec<u8>) -> Result<Self, DescriptorError> {
        let pool = DescriptorPool::decode(file_descriptor.as_slice())?;
        Ok(Self {
            state: Offline { pool },
        })
    }

    /// Lists all services defined in the local `DescriptorPool`.
    ///
    /// # Returns
    ///
    /// A list of fully qualified service names (e.g. `helloworld.Greeter`).
    pub fn list_services(&self) -> Vec<String> {
        self.state
            .pool
            .services()
            .map(|s| s.full_name().to_string())
            .collect()
    }

    /// Looks up a specific symbol in the local `DescriptorPool`.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The fully qualified name (Service, Message, or Enum).
    ///
    /// # Returns
    ///
    /// * `Some(Descriptor)` - The resolved descriptor if found.
    /// * `None` - If the symbol does not exist in the pool.
    pub fn get_descriptor_by_symbol(&self, symbol: &str) -> Option<Descriptor> {
        let pool = &self.state.pool;

        if let Some(descriptor) = pool.get_service_by_name(symbol) {
            return Some(Descriptor::ServiceDescriptor(descriptor));
        }
        if let Some(descriptor) = pool.get_message_by_name(symbol) {
            return Some(Descriptor::MessageDescriptor(descriptor));
        }
        if let Some(descriptor) = pool.get_enum_by_name(symbol) {
            return Some(Descriptor::EnumDescriptor(descriptor));
        }
        None
    }
}
