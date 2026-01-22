# Granc Core

[![Crates.io](https://img.shields.io/crates/v/granc_core.svg)](https://crates.io/crates/granc_core)
[![Documentation](https://docs.rs/granc_core/badge.svg)](https://docs.rs/granc_core)
[![License](https://img.shields.io/crates/l/granc_core.svg)](https://github.com/JasterV/granc/blob/main/LICENSE)

**`granc-core`** is the foundational library powering the [Granc CLI](https://crates.io/crates/granc). It provides a dynamic gRPC client capability that allows you to interact with *any* gRPC server without needing compile-time Protobuf code generation.

Instead of strictly typed Rust structs, this library bridges standard `serde_json::Value` payloads directly to Protobuf binary wire format at runtime.

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
granc_core = "0.2.3"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

## 🚀 High-Level Usage

The primary entry point is the [`GrancClient`]. It acts as an orchestrator that:

1. Connects to a gRPC server.
2. Resolves the schema (either from a local file or via Server Reflection).
3. Determines the method type (Unary, Server Streaming, etc.).
4. Execute the request using JSON.

### Example: Making a Dynamic Call

```rust
use granc_core::client::{GrancClient, DynamicRequest, DynamicResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let mut client = GrancClient::connect("http://localhost:50051").await?;

    // Prepare the request
    // If you don't provide a file_descriptor_set, the client will attempt
    // to fetch the schema from the server's reflection service automatically.
    let request = DynamicRequest {
        service: "helloworld.Greeter".to_string(),
        method: "SayHello".to_string(),
        body: json!({ "name": "World" }),
        headers: vec![],
        file_descriptor_set: None, // Uses Server Reflection
    };

    let response = client.dynamic(request).await?;

    match response {
        DynamicResponse::Unary(Ok(value)) => {
            println!("Response: {}", value);
        }
        DynamicResponse::Unary(Err(status)) => {
            eprintln!("gRPC Error: {:?}", status);
        }
        DynamicResponse::Streaming(Ok(stream)) => {
            for msg in stream {
                println!("Stream Msg: {:?}", msg);
            }
        }
        _ => eprintln!("Unexpected response type"),
    }

    Ok(())
}

```

## 🛠️ Internal Components

We expose the internal building blocks of `granc` for developers who need more granular control or want to build their own tools on top of our dynamic transport layer.

### 1. `GrpcClient` (Generic Transport)

Standard `tonic` clients are strongly typed (e.g., `client.say_hello(HelloRequest)`).
`GrpcClient` is a generic wrapper around `tonic::client::Grpc` that works strictly with `serde_json::Value` and `prost_reflect::MethodDescriptor`.

It handles the raw HTTP/2 path construction and metadata mapping, providing specific methods for all four gRPC access patterns:

* `unary`
* `server_streaming`
* `client_streaming`
* `bidirectional_streaming`

```rust
use granc_core::grpc::client::GrpcClient;
// You need a method_descriptor from prost_reflect::DescriptorPool
// let method_descriptor = ...; 

let mut grpc = GrpcClient::new(channel);
let result = grpc.unary(method_descriptor, json_value, headers).await?;

```

### 2. `JsonCodec`

The magic behind the dynamic serialization. This implementation of `tonic::codec::Codec` validates and transcodes JSON to Protobuf bytes (and vice versa) on the fly.

* **Encoder**: Validates `serde_json::Value` against the input `MessageDescriptor` and serializes it.
* **Decoder**: Deserializes bytes into a `DynamicMessage` and converts it back to `serde_json::Value`.

### 3. `ReflectionClient`

A client for `grpc.reflection.v1`. It enables runtime schema discovery.

The `ReflectionClient` is smart enough to handle dependencies. When you ask for a symbol (e.g., `my.package.Service`),
it recursively fetches the file defining that symbol and **all** its transitive imports, building a complete `prost_types::FileDescriptorSet` ready for use.

```rust
use granc_core::reflection::client::ReflectionClient;

let mut reflection = ReflectionClient::new(channel);
let fd_set = reflection.file_descriptor_set_by_symbol("my.package.Service").await?;
```

You can then build a `prost_reflect::DescriptorPool` with the returned `prost_types::FileDescriptorSet` to be able to inspect in detail the descriptor.

## ⚖️ License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
