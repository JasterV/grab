# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## `granc` - [0.3.2](https://github.com/JasterV/granc/compare/granc-v0.3.1...granc-v0.3.2) - 2026-01-22

### Fixed
- file-descriptor-set parsing fails because of type mismatch

## `granc-core` - [0.3.2](https://github.com/JasterV/granc/compare/granc-v0.3.1...granc-v0.3.2) - 2026-01-22

### Fixed
- file-descriptor-set parsing fails because of type mismatch

## `granc` - [0.3.1](https://github.com/JasterV/granc/compare/granc-v0.3.0...granc-v0.3.1) - 2026-01-22

### Other
- update Cargo.lock dependencies

## `granc_core` - [0.3.1](https://github.com/JasterV/granc/compare/granc_core-v0.3.0...granc_core-v0.3.1) - 2026-01-22

### Other
- update granc-core documentation

## `granc_core` - [0.3.0](https://github.com/JasterV/granc/compare/granc_core-v0.2.4...granc_core-v0.3.0) - 2026-01-22

- Separate reflection generation binary to not be published ([#20](https://github.com/JasterV/granc/pull/20))

## `granc` - [0.3.0](https://github.com/JasterV/granc/compare/granc_core-v0.2.4...granc_core-v0.3.0) - 2026-01-22

- Separate reflection generation binary to not be published ([#20](https://github.com/JasterV/granc/pull/20))

## `granc` - [0.2.4](https://github.com/JasterV/granc/compare/granc-v0.2.3...granc-v0.2.4) - 2026-01-22

- **Published granc-core** as a library crate `granc-core` ([#16](https://github.com/JasterV/granc/pull/16))

## `granc_core` - [0.2.4](https://github.com/JasterV/granc/compare/granc_core-v0.2.3...granc_core-v0.2.4) - 2026-01-22

- **Published granc-core** as a library crate `granc-core` ([#16](https://github.com/JasterV/granc/pull/16))

## `granc` - [0.2.3](https://github.com/JasterV/granc/compare/granc-v0.2.2...granc-v0.2.3) - 2026-01-21

- **Internal refactor**: Decouple ReflectionClient to possibly publish in a separate crate

## `granc` - [0.2.2](https://github.com/JasterV/granc/compare/granc-v0.2.1...granc-v0.2.2) - 2026-01-21

- Updated README.md

## `granc` - [0.2.1](https://github.com/JasterV/granc/compare/granc-v0.2.0...granc-v0.2.1) - 2026-01-21

- Updated README

## `granc` - [0.2.0](https://github.com/JasterV/granc/compare/granc-v0.1.0...granc-v0.2.0) - 2026-01-21

### Added

- **Automatic Reflection**: The tool now supports automatic reflection, trying to reach the reflection service in the server if the user doesn't provide a file descriptor binary ([#9](https://github.com/JasterV/granc/pull/9))

## `granc` - 0.1.0 2026-01-20

### Added

- **Dynamic gRPC Client**: Implemented a CLI that performs gRPC calls without generating Rust code, bridging JSON payloads to Protobuf binary format at runtime.
- **Schema Loading**: Support for loading Protobuf schemas dynamically from binary `FileDescriptorSet` (`.bin` or `.pb`) files.
- **Full Streaming Support**: Automatic dispatch for all four gRPC access patterns based on the method descriptor:
  - Unary (Single Request → Single Response)
  - Server Streaming (Single Request → Stream)
  - Client Streaming (Stream → Single Response)
  - Bidirectional Streaming (Stream → Stream)
- **JSON Transcoding**: Custom `tonic::Codec` implementation (`JsonCodec`) to validate and transcode `serde_json::Value` to/from Protobuf bytes on the fly.
- **Metadata Support**: Ability to attach custom headers/metadata to requests via the `-H` / `--header` flag.
- **Input Validation**: Fast-fail validation that checks if the provided JSON structure is valid before making the network request.
