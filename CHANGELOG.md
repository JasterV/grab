# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## `granc` - [0.7.1](https://github.com/JasterV/granc/compare/granc-v0.7.0...granc-v0.7.1) - 2026-02-06

- [feat] Add a new command to generate markdown documentation for gRPC services ([#46](https://github.com/JasterV/granc/pull/46))
- *(deps)* bump clap from 4.5.55 to 4.5.56 ([#45](https://github.com/JasterV/granc/pull/45))

## `granc_core` - [0.6.1](https://github.com/JasterV/granc/compare/granc_core-v0.6.0...granc_core-v0.6.1) - 2026-02-06

- Added `name`, `full_name`, and `package_name` methods to `Descriptor` to simplify access to descriptor metadata.

## `granc` - [0.7.0](https://github.com/JasterV/granc/compare/granc-v0.6.0...granc-v0.7.0) - 2026-01-28

- *(deps)* bump clap from 4.5.54 to 4.5.55 ([#36](https://github.com/JasterV/granc/pull/36))
- [fix] A URL should not be required for list and describe commands  ([#35](https://github.com/JasterV/granc/pull/35))
- [test] Added comprehensive tests for CLI argument parsing and validation.

## `granc_core` - [0.6.0](https://github.com/JasterV/granc/compare/granc_core-v0.5.0...granc_core-v0.6.0) - 2026-01-28

- [refactor] Now the `GrancClient` also provides a full offline state and other states have been renamed to be more idiomatic  ([#35](https://github.com/JasterV/granc/pull/35))

## `granc` - [0.6.0](https://github.com/JasterV/granc/compare/granc-v0.5.1...granc-v0.6.0) - 2026-01-27

- Make the `--file-descriptor-set` a global option for all commands, so reflection commands can also be executed against a local descriptor. ([#28](https://github.com/JasterV/granc/pull/28))

## `granc_core` - [0.5.0](https://github.com/JasterV/granc/compare/granc_core-v0.4.1...granc_core-v0.5.0) - 2026-01-27

- **Typestate design refactor**: The GrancClient has been refactored to support multiple states where invariants for each state are ensured by the compiler. ([#28](https://github.com/JasterV/granc/pull/28))
  - The GrancClient can be in either a `WithServerReflection` state or a `WithFileDescriptor` state, and both states have independent APIs (Async vs sync).

## `granc` - [0.5.1](https://github.com/JasterV/granc/compare/granc-v0.5.0...granc-v0.5.1) - 2026-01-24

### Other

- **Update deps**: Update `granc_core` to `0.4.1`

## `granc_core` - [0.4.1](https://github.com/JasterV/granc/compare/granc_core-v0.4.0...granc_core-v0.4.1) - 2026-01-27

### Other
- **Internal clean up**: We've replaced our own script that generated the Reflection client to use `tonic-reflection` instead. ([#29](https://github.com/JasterV/granc/pull/29))

## `granc` - [0.5.0](https://github.com/JasterV/granc/compare/granc-v0.2.4...granc-v0.5.0) - 2026-01-24

### Added

- **Introspection Commands**:
  - `list`: Lists all services available on the server (requires reflection).
  - `describe`: Lists all methods within a specific service, prints the Protobuf definition of a message type or show all the variants of an enum.
- **Formatted Output**: Added colored output for Protobuf definitions, JSON responses, and error messages.

### Changed

- **[BREAKING] New CLI Structure**: The CLI now enforces a `granc <URL> <COMMAND>` structure.
  - Previous implicit calls are now explicit: `granc http://... call <ENDPOINT> ...`.
  - The URL is now a global positional argument required for all commands.

## `granc_core` - [0.4.0](https://github.com/JasterV/granc/compare/granc_core-v0.3.1...granc_core-v0.4.0) - 2026-01-24

### Added

- **Introspection APIs**: Added `list_services` and `get_descriptor_by_symbol` to `GrancClient`.
- **Reflection Support**: Updated `ReflectionClient` to support the `ListServices` reflection method.

### Changed

- **Error Handling Refactor**: Overhauled error types to be more specific per method (`GetDescriptoError`, `ListServicesError`) and reduced internal duplication.

## `granc` - [0.4.0](https://github.com/JasterV/granc/compare/granc-v0.2.4...granc-v0.4.0) - 2026-01-22

- Made a mistake publishing `granc 0.3` and introduced bugs, `granc 0.4` fixes them and its the first working version after `0.2.4`.

## `granc_core` - [0.3.1](https://github.com/JasterV/granc/compare/granc_core-v0.3.0...granc_core-v0.3.1) - 2026-01-22

### Other

- Update granc-core documentation

## `granc_core` - [0.3.0](https://github.com/JasterV/granc/compare/granc_core-v0.2.4...granc_core-v0.3.0) - 2026-01-22

- Fix: separate reflection generation binary to not be published ([#20](https://github.com/JasterV/granc/pull/20))

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
