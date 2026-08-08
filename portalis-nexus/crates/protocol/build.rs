use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");
    let common = proto_root.join("portalis/protocol/v1/common.proto");
    let connection = proto_root.join("portalis/protocol/v1/connection.proto");
    let identity = proto_root.join("portalis/protocol/v1/identity.proto");
    let descriptor = PathBuf::from(std::env::var("OUT_DIR")?).join("portalis_nexus.bin");

    println!("cargo:rerun-if-changed={}", common.display());
    println!("cargo:rerun-if-changed={}", connection.display());
    println!("cargo:rerun-if-changed={}", identity.display());

    let mut config = prost_build::Config::new();
    config
        .protoc_executable(protoc_bin_vendored::protoc_bin_path()?)
        .file_descriptor_set_path(descriptor)
        .compile_protos(&[common, connection, identity], &[proto_root])?;

    Ok(())
}
