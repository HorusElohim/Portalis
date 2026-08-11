use std::error::Error;
use std::path::{Path, PathBuf};

/// Compiles every schema under `proto/`.
///
/// The set is discovered rather than listed, so adding a domain schema does
/// not mean editing this file. Sorting keeps the build reproducible.
fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");
    let descriptor = PathBuf::from(std::env::var("OUT_DIR")?).join("portalis_nexus.bin");

    let mut schemas = Vec::new();
    collect_schemas(&proto_root, &mut schemas)?;
    schemas.sort();
    if schemas.is_empty() {
        return Err(format!("no .proto files under {}", proto_root.display()).into());
    }

    println!("cargo:rerun-if-changed={}", proto_root.display());
    for schema in &schemas {
        println!("cargo:rerun-if-changed={}", schema.display());
    }

    let mut config = prost_build::Config::new();
    config
        .protoc_executable(protoc_bin_vendored::protoc_bin_path()?)
        .file_descriptor_set_path(descriptor)
        .compile_protos(&schemas, &[proto_root])?;

    Ok(())
}

fn collect_schemas(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_schemas(&path, found)?;
        } else if path.extension().is_some_and(|kind| kind == "proto") {
            found.push(path);
        }
    }
    Ok(())
}
