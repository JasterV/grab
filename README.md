# gRab 🦀

> ⚠️ **Status: Experimental**
>
> This project is currently in a **highly experimental phase**. It is a working prototype intended for testing and development purposes. APIs, command-line arguments, and internal logic are subject to breaking changes. Please use with caution.

**gRab** (gRPC + Crab) is a lightweight, dynamic gRPC CLI tool written in Rust.

It allows you to make gRPC calls to any server using simple JSON payloads, without needing to compile the specific Protobuf files into the client. By loading a `FileDescriptorSet` at runtime, gRab acts as a bridge between human-readable JSON and binary Protobuf wire format.

It is heavily inspired by tools like `grpcurl` but built to leverage the safety and performance of the Rust ecosystem (Tonic + Prost).

## 🚀 Features

* **Dynamic Encoding/Decoding**: Transcodes JSON to Protobuf (and vice versa) on the fly using `prost-reflect`.
* **Smart Dispatch**: Automatically detects if a call is Unary, Server Streaming, Client Streaming, or Bidirectional based on the descriptor.
* **Fast Fail Validation**: Validates your JSON *before* hitting the network.
* **Zero Compilation Dependencies**: Does not require generating Rust code for your protos. Just point to a descriptor file.
* **Metadata Support**: Easily attach custom headers (authorization, tracing) to your requests.
* **Tonic 0.14**: Built on the latest stable Rust gRPC stack.

## 📦 Installation

### From Source

Ensure you have Rust and Cargo installed.

```bash
git clone https://github.com/JasterV/grab
cd grab
cargo install --path .
```

## 🛠️ Prerequisites: Generating Descriptors

To use gRab, you currently need a binary **FileDescriptorSet** (`.bin` or `.pb`). This file contains the schema definitions for your services.

You can generate this using the standard `protoc` compiler:

```bash
# Generate descriptor.bin including all imports
protoc \
    --include_imports \
    --descriptor_set_out=descriptor.bin \
    --proto_path=. \
    my_service.proto

```

> **Note**: The `--include_imports` flag is crucial. It ensures that types defined in imported files (like `google/protobuf/timestamp.proto`) are available for reflection.

## 📖 Usage

**Syntax:**

```bash
grab [OPTIONS] <URL> <METHOD>

```

### Arguments

| Argument | Description | Required |
| --- | --- | --- |
| `<URL>` | Server address (e.g., `http://[::1]:50051`). | **Yes** |
| `<METHOD>` | Fully qualified method name (e.g., `my.package.Service/Method`). | **Yes** |

### Options

| Flag | Short | Description | Required |
| --- | --- | --- | --- |
| `--proto-set` |  | Path to the binary FileDescriptorSet (`.bin`). | **Yes** |
| `--body` |  | The request body in JSON format. | **Yes** |
| `--header` | `-H` | Custom header `key:value`. Can be used multiple times. | No |

### JSON Body Format

* **Unary / Server Streaming**: Provide a single JSON object `{ ... }`.
* **Client / Bidirectional Streaming**: Provide a JSON array of objects `[ { ... }, { ... } ]`.

### Examples

**1. Unary Call**

```bash
grab \
  --proto-set ./descriptor.bin \
  --body '{"name": "Ferris"}' \
  http://localhost:50051 \
  helloworld.Greeter/SayHello
```

**2. Bidirectional Streaming (Chat)**

```bash
grab \
  --proto-set ./descriptor.bin \
  --body '[{"text": "Hello"}, {"text": "How are you?"}]' \
  -H "authorization: Bearer token123" \
  http://localhost:50051 \
  chat.ChatService/StreamMessages
```

## 🔮 Roadmap

* **Automatic Server Reflection**: We are working on removing the requirement for the `--proto-set` file. Future versions will support fetching the schema directly from servers that have the [gRPC Server Reflection Protocol](https://github.com/grpc/grpc/blob/master/doc/server-reflection.md) enabled.
* **Interactive Mode**: A REPL for streaming requests interactively.
* **Pretty Printing**: Enhanced colored output for JSON responses.

## ⚠️ Common Errors

**1. `Service 'x' not found**`

* **Cause:** The service name in the command does not match the package defined in your proto file.
* **Fix:** Check your `.proto` file. If it has `package my.app;` and `service API {}`, the full name is `my.app.API`.

**2. `Method 'y' not found in service 'x'**`

* **Cause:** Typo in the method name or the method doesn't exist.
* **Fix:** Ensure case sensitivity matches (e.g., `GetUser` vs `getUser`).

**3. `h2 protocol error**`

* **Cause:** This often occurs when the JSON payload fails to encode *after* the connection has already been established, or the server rejected the stream structure.
* **Fix:** Double-check your JSON payload against the Protobuf schema.

## 🤝 Contributing

Contributions are welcome! Please run the Makefile checks before submitting a PR:

```bash
cargo make      # Formats, lints, and builds
```

## 📄 License

Licensed under either of:

* Apache License, Version 2.0 ([LICENSE-APACHE](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](http://opensource.org/licenses/MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
