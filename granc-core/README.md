# Granc Core

[![Crates.io](https://img.shields.io/crates/v/granc_core.svg)](https://crates.io/crates/granc_core)
[![Documentation](https://docs.rs/granc_core/badge.svg)](https://docs.rs/granc_core)
[![License](https://img.shields.io/crates/l/granc_core.svg)](https://github.com/JasterV/granc/blob/main/LICENSE)

**`granc-core`** is the foundational library powering the [Granc CLI](https://crates.io/crates/granc). It provides a dynamic gRPC client capability that allows you to interact with *any* gRPC server without needing compile-time Protobuf code generation.

Instead of strictly typed Rust structs, this library bridges standard `serde_json::Value` payloads directly to Protobuf binary wire format at runtime.

## 🚀 High-Level Usage

The primary entry point is the [`GrancClient`]. It uses a **Typestate Pattern** to ensure safety and correctness regarding how the Protobuf schema is resolved. There are three distinct states:

1. **[`Online`]**: Connected to a server, uses Server Reflection (Async introspection).
2. **[`OnlineWithoutReflection`]**: Connected to a server, uses a local `FileDescriptorSet` (Sync introspection).
3. **[`Offline`]**: Disconnected, uses a local `FileDescriptorSet` (Sync introspection).

### 1. Online (Server Reflection)

This is the default state when you connect. The client queries the server's reflection endpoint to dynamically discover services and message formats.

```rust
use granc_core::client::{GrancClient, DynamicRequest, DynamicResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect (Starts in 'Online' state)
    let mut client = GrancClient::connect("http://localhost:50051").await?;

    // 2. Introspection (Async via Reflection)
    let services = client.list_services().await?;
    println!("Server services: {:?}", services);

    // 3. Dynamic Call
    let request = DynamicRequest {
        service: "helloworld.Greeter".to_string(),
        method: "SayHello".to_string(),
        body: json!({ "name": "Ferris" }),
        headers: vec![],
    };

    // Schema is fetched automatically from the server
    let response = client.dynamic(request).await?;
    println!("{:?}", response);

    Ok(())
}

```

### 2. OnlineWithoutReflection (Local Schema)

Use this state if you are connecting to a server that does not support reflection, or if you want to enforce a specific schema version from a local file.

```rust
use granc_core::client::GrancClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GrancClient::connect("http://localhost:50051").await?;
    let descriptor_bytes = std::fs::read("descriptor.bin")?;

    // Transition state: Online -> OnlineWithoutReflection
    let mut client = client.with_file_descriptor(descriptor_bytes)?;

    // Introspection is now SYNCHRONOUS (in-memory)
    let services = client.list_services(); 
    println!("Local services: {:?}", services);

    // Dynamic calls use the local schema to encode/decode
    // client.dynamic(req).await?;

    Ok(())
}

```

### 3. Offline (Introspection Only)

This state is useful for building tools that need to inspect `.bin` descriptor files without establishing a network connection.

```rust
use granc_core::client::GrancClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_bytes = std::fs::read("descriptor.bin")?;

    // Create directly in 'Offline' state
    let client = GrancClient::offline(descriptor_bytes)?;

    // Sync introspection methods
    let services = client.list_services();
    
    if let Some(descriptor) = client.get_descriptor_by_symbol("helloworld.Greeter") {
        println!("Found service: {:?}", descriptor);
    }

    // Note: client.dynamic() is NOT available in this state.
    Ok(())
}

```

## 🛠️ Internal Components

We expose the internal building blocks of `granc` for developers who need more granular control or want to build their own tools on top of our dynamic transport layer.

### 1. `GrpcClient` (Generic Transport)

Standard `tonic` clients are strongly typed. `GrpcClient` is a generic wrapper around `tonic::client::Grpc` that works strictly with `serde_json::Value` and `prost_reflect::MethodDescriptor`. It handles the raw HTTP/2 path construction and metadata mapping.

### 2. `JsonCodec`

The magic behind the dynamic serialization. This implementation of `tonic::codec::Codec` validates and transcodes JSON to Protobuf bytes (and vice versa) on the fly.

### 3. `ReflectionClient`

A robust client for `grpc.reflection.v1`. It automatically handles transitive dependency resolution, recursively fetching all imported files to build a complete, self-contained `FileDescriptorSet`.

## ⚖️ License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
