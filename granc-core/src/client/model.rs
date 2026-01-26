use prost_reflect::{EnumDescriptor, MessageDescriptor, ServiceDescriptor};

/// A request object encapsulating all necessary information to perform a dynamic gRPC call.
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
pub enum DynamicResponse {
    /// A single response message (for Unary and Client Streaming calls).
    Unary(Result<serde_json::Value, tonic::Status>),
    /// A stream of response messages (for Server Streaming and Bidirectional calls).
    Streaming(Result<Vec<Result<serde_json::Value, tonic::Status>>, tonic::Status>),
}

/// A file descriptor of either a message, service or enum
#[derive(Debug, Clone)]
pub enum Descriptor {
    MessageDescriptor(MessageDescriptor),
    ServiceDescriptor(ServiceDescriptor),
    EnumDescriptor(EnumDescriptor),
}

impl Descriptor {
    pub fn message_descriptor(&self) -> Option<&MessageDescriptor> {
        match self {
            Descriptor::MessageDescriptor(message_descriptor) => Some(message_descriptor),
            _ => None,
        }
    }

    pub fn service_descriptor(&self) -> Option<&ServiceDescriptor> {
        match self {
            Descriptor::ServiceDescriptor(service_descriptor) => Some(service_descriptor),
            _ => None,
        }
    }

    pub fn enum_descriptor(&self) -> Option<&EnumDescriptor> {
        match self {
            Descriptor::EnumDescriptor(enum_descriptor) => Some(enum_descriptor),
            _ => None,
        }
    }
}
