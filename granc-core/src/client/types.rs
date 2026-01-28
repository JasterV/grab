use prost_reflect::{EnumDescriptor, MessageDescriptor, ServiceDescriptor};
use std::fmt::Debug;

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
