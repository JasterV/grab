# gRab 🦀

> ⚠️ **Status: Experimental**
>
> This project is currently in a **highly experimental phase**. It is a working prototype intended for testing and development purposes. APIs, command-line arguments, and internal logic are subject to breaking changes. Please use with caution.

**gRab** (gRPC + Crab) is a lightweight, dynamic gRPC CLI tool written in Rust.

It allows you to make gRPC calls to any server using simple JSON payloads, without needing to compile the specific Protobuf files into the client. By loading a `FileDescriptorSet` at runtime, gRab acts as a bridge between human-readable JSON and binary Protobuf wire format.

It is heavily inspired by tools like `grpcurl` but built to leverage the safety and performance of the Rust ecosystem (Tonic + Prost).

## 🚀 Features

* **Dynamic Encoding/Decoding**: Transcodes JSON to Protobuf (and vice versa) on the fly using `prost-reflect`.
* **Fast Fail Validation**: Validates your JSON against the schema *before* hitting the network.
* **Zero Compilation Dependencies**: Does not require generating Rust code for your protos. Just point to a descriptor file.
* **Metadata Support**: Easily attach custom headers (authorization, tracing) to your requests.
* **Tonic 0.14**: Built on the latest stable Rust gRPC stack.

## 📦 Installation

### From Source

Ensure you have Rust and Cargo installed.

```bash
git clone [https://github.com/your-username/grab.git](https://github.com/your-username/grab.git)
cd grab
cargo install --path .

```

## 🛠️ Prerequisites: Generating Descriptors

To use gRab, you need a binary **FileDescriptorSet** (`.bin` or `.pb`). This file contains the schema definitions for your services.

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

The basic syntax is:

```bash
grab unary [OPTIONS]
```

### Options

| Flag | Short | Description | Required |
| --- | --- | --- | --- |
| `--descriptor` | `-d` | Path to the binary FileDescriptorSet. | Yes |
| `--addr` | `-a` | Server address (e.g., `http://[::1]:50051`). | Yes |
| `--service` | `-s` | Fully qualified service name (e.g., `my.pkg.UserService`). | Yes |
| `--method` | `-m` | Method name (e.g., `CreateUser`). | Yes |
| `--json` | `-j` | The request body in JSON format. | Yes |
| `--header` | `-H` | Custom header `key:value`. Can be used multiple times. | No |

### Example

Assuming you have a `Greeter` service running locally on port 50051:

```bash
grab unary \
  --descriptor ./descriptor.bin \
  --addr http://localhost:50051 \
  --service helloworld.Greeter \
  --method SayHello \
  --json "$(< payload.json)" \
  -H "authorization: Bearer my-token"
```

**Output:**

```json
{
  "message": "Hello Ferris"
}
```

## ⚠️ Common Errors

**1. `Service 'x' not found**`

* **Cause:** The service name in the command does not match the package defined in your proto file.
* **Fix:** Check your `.proto` file. If it has `package my.app;` and `service API {}`, the full name is `my.app.API`.

**2. `Method 'y' not found in service 'x'**`

* **Cause:** Typo in the method name or the method doesn't exist.
* **Fix:** Ensure case sensitivity matches (e.g., `GetUser` vs `getUser`).

**3. `JSON structure does not match Protobuf schema**`

* **Cause:** Your JSON payload has fields that don't exist in the proto message, or types are incorrect (e.g., string instead of int).
* **Fix:** gRab validates this locally. Check your field names and types against the proto definition.

**4. `h2 protocol error: error reading a body from connection`** (or `h2 protocol error`)

* **Cause:** This often occurs when the JSON payload fails to encode *after* the connection has already been established. The client aborts the request stream mid-flight, causing the server to register a protocol error instead of a validation error.
* **Fix:** Double-check your JSON payload against the Protobuf schema. Ensure all field names are correct and match the expected types (e.g., sending a string for an integer field, or using a field name that doesn't exist).

## 🤝 Contributing

Contributions are welcome! Please run the Makefile checks before submitting a PR:

```bash
cargo make      # Formats, lints, and builds
```

## 📄 License

Licensed under either of:

* Apache License, Version 2.0 ([LICENSE-APACHE](https://www.google.com/search?q=LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](https://www.google.com/search?q=LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
