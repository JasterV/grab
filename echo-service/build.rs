use std::env::var;
use std::io::Result;

fn main() -> Result<()> {
    // List of proto files containing a message definition
    let proto_files = &[
        // Services
        "proto/echo.proto",
    ];

    // Name of the folder containing the proto definitions
    let proto_folder = "proto";
    let out_dir = var("OUT_DIR").expect("Missing OUT_DIR environment variable");
    let descriptors_path = format!("{}/descriptors.bin", out_dir);

    tonic_prost_build::configure()
        .file_descriptor_set_path(descriptors_path)
        .protoc_arg("--experimental_allow_proto3_optional")
        .build_client(false)
        .compile_protos(proto_files, &[proto_folder])
        .unwrap();

    Ok(())
}
