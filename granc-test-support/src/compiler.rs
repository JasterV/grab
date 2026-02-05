use prost::Message;
use prost_types::FileDescriptorSet;
use std::fs;

/// Compiles inline proto strings into a DescriptorPool at runtime.
///
/// # Arguments
/// * `files` - A list of tuples (filename, content). E.g. `[("test.proto", "syntax=...")]`
pub fn compile_protos(files: &[(&str, &str)]) -> FileDescriptorSet {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let descriptor_path = temp_dir.path().join("descriptor.bin");
    let proto_dir = temp_dir.path().join("protos");

    fs::create_dir(&proto_dir).expect("Failed to create protos dir");

    let paths: Vec<_> = files
        .into_iter()
        .map(|(name, content)| {
            let path = proto_dir.join(name);
            fs::write(&path, content).expect("Failed to write proto file");
            path
        })
        .collect();

    let mut config = prost_build::Config::new();
    config.file_descriptor_set_path(&descriptor_path);
    config.out_dir(temp_dir.path());
    config
        .compile_protos(&paths, &[proto_dir])
        .expect("Failed to compile protos");

    let bytes = fs::read(descriptor_path).expect("Failed to read descriptor set");

    FileDescriptorSet::decode(bytes.as_slice()).expect("Failed to decode File descriptor set")
}
