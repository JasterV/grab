use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating Reflection Service types...");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir = manifest_dir.join("src/reflection/generated");

    let proto_file = manifest_dir.join("proto/reflection.proto");
    let proto_folder = manifest_dir.join("proto");

    if !out_dir.exists() {
        fs::create_dir_all(&out_dir)?;
    }

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(&out_dir)
        .compile_protos(&[proto_file], &[proto_folder])
        .unwrap();

    println!("Done! Generated files are in src/reflection/generated");

    Ok(())
}
