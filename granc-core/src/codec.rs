//! # JSON <-> Protobuf Codec
//!
//! This module implements `tonic::codec::Codec` to enable `tonic` to transport `serde_json::Value`
//! directly, bypassing the need for generated Rust structs.
//!
//! ## How it works
//!
//! 1. **Encoder (JSON -> Proto)**:
//!    - Takes a `serde_json::Value`.
//!    - Uses `prost_reflect::DynamicMessage` to validate the JSON against the input `MessageDescriptor`.
//!    - Serializes the valid message into the generic gRPC byte buffer.
//!
//! 2. **Decoder (Proto -> JSON)**:
//!    - Reads raw bytes from the wire.
//!    - Decodes them into a `DynamicMessage` using the output `MessageDescriptor`.
//!    - Converts the message back into a `serde_json::Value` for the CLI to print.
use prost::Message;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
};

/// A custom Codec that bridges `serde_json::Value` and Protobuf binary format.
///
/// It holds the descriptors (schemas) for both the request and the response messages,
/// allowing it to perform dynamic serialization.
pub struct JsonCodec {
    /// Schema for the input message.
    req_desc: MessageDescriptor,
    /// Schema for the output message.
    res_desc: MessageDescriptor,
}

impl JsonCodec {
    /// Creates a new `JsonCodec`.
    ///
    /// # Arguments
    /// * `req_desc` - Descriptor for the request message type.
    /// * `res_desc` - Descriptor for the response message type.    
    pub fn new(req_desc: MessageDescriptor, res_desc: MessageDescriptor) -> Self {
        Self { req_desc, res_desc }
    }
}

impl Codec for JsonCodec {
    type Encode = serde_json::Value;
    type Decode = serde_json::Value;

    type Encoder = JsonEncoder;
    type Decoder = JsonDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        JsonEncoder(self.req_desc.clone())
    }

    fn decoder(&mut self) -> Self::Decoder {
        JsonDecoder(self.res_desc.clone())
    }
}

/// Responsible for encoding a JSON value into Protobuf bytes.
pub struct JsonEncoder(MessageDescriptor);

impl Encoder for JsonEncoder {
    type Item = serde_json::Value;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        // DynamicMessage::deserialize accepts any Serde Deserializer.
        // serde_json::Value implements IntoDeserializer, so we can pass it directly.
        let msg = DynamicMessage::deserialize(self.0.clone(), item).map_err(|e| {
            Status::invalid_argument(format!(
                "JSON structure does not match Protobuf schema: {}",
                e
            ))
        })?;

        msg.encode_raw(dst);
        Ok(())
    }
}

/// Responsible for decoding Protobuf bytes into a JSON value.
pub struct JsonDecoder(MessageDescriptor);

impl Decoder for JsonDecoder {
    type Item = serde_json::Value;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        // 1. Decode Bytes -> DynamicMessage
        let mut msg = DynamicMessage::new(self.0.clone());
        msg.merge(src)
            .map_err(|e| Status::internal(format!("Failed to decode Protobuf bytes: {}", e)))?;

        // 2. DynamicMessage -> serde_json::Value
        // We convert the DynamicMessage into a Value structure.
        // This is efficient and keeps the Client working with structured data.
        let value = serde_json::to_value(&msg)
            .map_err(|e| Status::internal(format!("Failed to map response to JSON: {}", e)))?;

        Ok(Some(value))
    }
}
