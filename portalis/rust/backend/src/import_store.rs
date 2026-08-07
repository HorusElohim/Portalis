//! Durable native publication jobs.
//!
//! A large copy or torrent hash can outlive one app process. Persisting only
//! lightweight source descriptors lets the next process resume the Rust job
//! without ever serialising file contents through Flutter or FFI.

use serde::{Deserialize, Serialize};

use crate::torrent::SourceFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImportRecord {
    pub(crate) collection_id: String,
    pub(crate) label: String,
    pub(crate) batch_name: String,
    pub(crate) files: Vec<SourceFile>,
    pub(crate) total_bytes: u64,
    pub(crate) error: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct ImportStore {
    jobs: Vec<ImportRecord>,
}

fn vault() -> crate::vault::Vault {
    crate::vault::Vault::named("imports.json")
}

pub(crate) fn load() -> anyhow::Result<Vec<ImportRecord>> {
    Ok(vault().read::<ImportStore>()?.unwrap_or_default().jobs)
}

pub(crate) fn save(jobs: Vec<ImportRecord>) -> anyhow::Result<()> {
    vault().write(&ImportStore { jobs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_descriptors_survive_a_restart_without_file_bytes() {
        let temp = crate::paths::redirect_to_temp();
        let record = ImportRecord {
            collection_id: "collection".into(),
            label: "Holiday".into(),
            batch_name: "holiday-batch".into(),
            files: vec![SourceFile {
                name: "movie.mkv".into(),
                path: "D:\\Media\\movie.mkv".into(),
                length_bytes: Some(8_000_000_000),
            }],
            total_bytes: 8_000_000_000,
            error: None,
        };

        save(vec![record]).unwrap();
        let restored = load().unwrap();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].total_bytes, 8_000_000_000);
        assert_eq!(restored[0].files[0].path, "D:\\Media\\movie.mkv");
        assert!(temp.path("imports.json").is_file());
    }
}
