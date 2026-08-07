//! Durable source descriptors for torrents that seed directly from a gallery.

use serde::{Deserialize, Serialize};

use crate::torrent::SourceFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LinkedSourceRecord {
    pub(crate) info_hash: String,
    pub(crate) torrent_bytes: Vec<u8>,
    pub(crate) sources: Vec<SourceFile>,
}

#[derive(Default, Serialize, Deserialize)]
struct LinkedSourceStore {
    records: Vec<LinkedSourceRecord>,
}

fn vault() -> crate::vault::Vault {
    crate::vault::Vault::named("linked-sources.json")
}

pub(crate) fn load() -> anyhow::Result<Vec<LinkedSourceRecord>> {
    Ok(vault()
        .read::<LinkedSourceStore>()?
        .unwrap_or_default()
        .records)
}

pub(crate) fn upsert(record: LinkedSourceRecord) -> anyhow::Result<()> {
    let mut records = load()?;
    records.retain(|existing| existing.info_hash != record.info_hash);
    records.push(record);
    vault().write(&LinkedSourceStore { records })
}

pub(crate) fn remove(info_hash: &str) -> anyhow::Result<()> {
    let mut records = load()?;
    records.retain(|record| record.info_hash != info_hash);
    vault().write(&LinkedSourceStore { records })
}

pub(crate) fn paths_for(info_hash: &str) -> Vec<String> {
    load()
        .ok()
        .and_then(|records| {
            records
                .into_iter()
                .find(|record| record.info_hash.eq_ignore_ascii_case(info_hash))
        })
        .map(|record| {
            record
                .sources
                .into_iter()
                .map(|source| source.path)
                .collect()
        })
        .unwrap_or_default()
}
