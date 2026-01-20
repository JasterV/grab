# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## `grab` - [0.1.0](https://github.com/JasterV/grab/compare/grab-v0.1.0...grab-v0.1.1) - 2026-01-20

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
