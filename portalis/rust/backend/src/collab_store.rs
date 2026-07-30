//! Local persistence for collab collections (`collections.json`) —
//! deliberately its own top-level module, *not* listed in
//! `flutter_rust_bridge`'s `--rust-input` (see `tool/frb_build.sh`), for
//! the same reason `domain` isn't listed there either: FRB's codegen
//! bridges every struct textually present within a listed module's
//! subtree, regardless of Rust visibility. A private `PersistedCollection`
//! struct living inside `crate::collections` would get swept up and FRB would
//! try (and fail — its fields aren't `pub`) to generate bridging code for
//! it, even though nothing outside this crate ever needs it. Keeping the
//! persisted-DTO types in a module never named in `--rust-input`
//! sidesteps the problem entirely.
//!
//! The `Persisted*` types double as the manifest-sync **wire format**
//! (see `collab_sync.rs`): what's good for surviving a restart — a
//! self-contained, signature-carrying, hex-encoded snapshot of an entry —
//! is exactly what's good for handing that entry to a peer. Entries are
//! re-verified on the way back in either way (`Manifest::add` checks the
//! signature), so a tampered file on disk and a malicious peer are
//! rejected by the same code path.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};

use crate::domain::collaborator::{Collaborator, Role};
use crate::domain::collection::{Collection, CollectionId};
use crate::domain::identity::DeviceId;
use crate::domain::invite::InviteSecret;
use crate::domain::manifest::{InfoHash, Manifest, ManifestEntry};
use crate::log::clog;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PersistedManifestEntry {
    pub(crate) info_hash_hex: String,
    pub(crate) name: String,
    pub(crate) thumbnail_hash_hex: Option<String>,
    pub(crate) added_by_hex: String,
    pub(crate) added_at_unix_ms: i64,
    pub(crate) signature_hex: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PersistedCollaborator {
    pub(crate) device_id_hex: String,
    pub(crate) display_name: String,
    pub(crate) is_admin: bool,
    pub(crate) joined_at_unix_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct PersistedCollection {
    id: String,
    name: String,
    invite_secret_hex: String,
    collaborators: Vec<PersistedCollaborator>,
    manifest: Vec<PersistedManifestEntry>,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedStore {
    collections: Vec<PersistedCollection>,
}

pub(crate) fn entry_to_persisted(e: &ManifestEntry) -> PersistedManifestEntry {
    PersistedManifestEntry {
        info_hash_hex: e.info_hash.to_hex(),
        name: e.name.clone(),
        thumbnail_hash_hex: e.thumbnail_hash.map(hex::encode),
        added_by_hex: e.added_by.to_hex(),
        added_at_unix_ms: e.added_at_unix_ms,
        signature_hex: hex::encode(e.signature_bytes()),
    }
}

/// Rebuilds the entry as-signed. This does **not** verify it — the one
/// place entries enter a [`Manifest`] (`Manifest::add`) does, identically
/// for entries loaded from disk and entries received from a peer.
pub(crate) fn entry_from_persisted(
    e: &PersistedManifestEntry,
) -> anyhow::Result<ManifestEntry> {
    let info_hash_bytes: [u8; 20] = hex::decode(&e.info_hash_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("stored info hash is not 20 bytes"))?;
    let thumbnail_hash = e
        .thumbnail_hash_hex
        .as_ref()
        .map(|h| -> anyhow::Result<[u8; 32]> {
            hex::decode(h)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("stored thumbnail hash is not 32 bytes"))
        })
        .transpose()?;
    let signature_bytes: [u8; 64] = hex::decode(&e.signature_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("stored signature is not 64 bytes"))?;
    Ok(ManifestEntry::from_signed_parts(
        InfoHash::from_bytes(info_hash_bytes),
        e.name.clone(),
        thumbnail_hash,
        DeviceId::from_hex(&e.added_by_hex)?,
        e.added_at_unix_ms,
        Signature::from_bytes(&signature_bytes),
    ))
}

pub(crate) fn collaborator_to_persisted(c: &Collaborator) -> PersistedCollaborator {
    PersistedCollaborator {
        device_id_hex: c.device_id.to_hex(),
        display_name: c.display_name.clone(),
        is_admin: c.is_admin(),
        joined_at_unix_ms: c.joined_at_unix_ms,
    }
}

pub(crate) fn collaborator_from_persisted(
    c: &PersistedCollaborator,
) -> anyhow::Result<Collaborator> {
    Ok(Collaborator::new(
        DeviceId::from_hex(&c.device_id_hex)?,
        c.display_name.clone(),
        if c.is_admin { Role::Admin } else { Role::Member },
        c.joined_at_unix_ms,
    ))
}

fn store_file() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    let path = base.join("Portalis").join("collections.json");
    clog!("collab_store", "store_file: {path:?}");
    path
}

fn to_persisted(collection: &Collection) -> PersistedCollection {
    PersistedCollection {
        id: collection.id.to_string(),
        name: collection.name.clone(),
        invite_secret_hex: collection.invite_secret_hex(),
        collaborators: collection
            .collaborators
            .iter()
            .map(collaborator_to_persisted)
            .collect(),
        manifest: collection.manifest().entries().map(entry_to_persisted).collect(),
    }
}

fn from_persisted(persisted: &PersistedCollection) -> anyhow::Result<Collection> {
    let id = CollectionId::from_string(&persisted.id)?;
    let invite_secret = InviteSecret::from_hex(&persisted.invite_secret_hex)?;
    let collaborators = persisted
        .collaborators
        .iter()
        .map(collaborator_from_persisted)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut manifest = Manifest::new();
    for e in &persisted.manifest {
        manifest.add(entry_from_persisted(e)?);
    }
    Ok(Collection::from_parts(
        id,
        persisted.name.clone(),
        invite_secret,
        collaborators,
        manifest,
    ))
}

fn load() -> anyhow::Result<Vec<Collection>> {
    let path = store_file();
    let Ok(bytes) = std::fs::read(&path) else {
        clog!("collab_store", "load: no file yet at {path:?}, starting empty");
        return Ok(Vec::new());
    };
    let persisted: PersistedStore =
        serde_json::from_slice(&bytes).context("parsing collections.json")?;
    let result: anyhow::Result<Vec<Collection>> =
        persisted.collections.iter().map(from_persisted).collect();
    match &result {
        Ok(collections) => clog!(
            "collab_store",
            "load: {} collection(s) from {path:?}: {:?}",
            collections.len(),
            collections.iter().map(|c| (c.id.to_string(), c.name.clone())).collect::<Vec<_>>()
        ),
        Err(e) => clog!("collab_store", "load: failed to parse {path:?}: {e:?}"),
    }
    result
}

/// Persists the store **atomically**: serialise to a sibling temp file, then
/// rename it over the real one.
///
/// A plain `fs::write` opens with `O_TRUNC`, so the existing file is destroyed
/// the instant the write begins and any later failure — a full disk, a crash,
/// a force-quit — leaves nothing behind. That is not hypothetical: this
/// project's own dev machine filled its disk mid-session and the store was
/// found empty afterwards. `rename` within the same directory is atomic, so a
/// reader sees either the complete old file or the complete new one, and a
/// failed write leaves the original untouched.
fn save(collections: &[Collection]) -> anyhow::Result<()> {
    let path = store_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {parent:?}"))?;
    }
    let persisted = PersistedStore {
        collections: collections.iter().map(to_persisted).collect(),
    };
    let bytes = serde_json::to_vec_pretty(&persisted)?;

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes).with_context(|| format!("writing {tmp:?}"))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("replacing {path:?} with {tmp:?}"))?;
    clog!(
        "collab_store",
        "save: wrote {} collection(s), {} bytes, to {path:?}",
        collections.len(),
        bytes.len()
    );
    Ok(())
}

/// The one shared in-memory copy of every collab collection, lazily loaded
/// from disk on first use. Shared between `collections.rs` (the FRB commands)
/// and `collab_sync.rs` (the peer-sync listener) — both mutate collections
/// through here so a sync arriving mid-command can't clobber a half-written
/// `collections.json`.
static STORE: Mutex<Option<Vec<Collection>>> = Mutex::new(None);

/// Lazily loads the store on first access. Callers go through
/// [`read_store`] or [`with_store`] rather than this.
fn lock_loaded<R>(
    f: impl FnOnce(&mut Vec<Collection>) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let mut guard = STORE.lock().unwrap();
    if guard.is_none() {
        clog!("collab_store", "cold start, loading from disk");
        *guard = Some(load()?);
    }
    f(guard.as_mut().unwrap())
}

/// Read-only access — **never writes to disk**.
///
/// Use this for anything that only inspects the store. It used to be
/// impossible: every access went through [`with_store`], which saved
/// unconditionally, so merely *listing* collections rewrote
/// `collections.json`. With the UI polling once a second that meant a
/// rewrite per second forever, which is both pointless and a standing
/// opportunity for a failed write to land on good data.
pub(crate) fn read_store<R>(
    f: impl FnOnce(&Vec<Collection>) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    lock_loaded(|collections| f(collections))
}

/// Locks the store, runs `f`, then persists. `f` must be synchronous — any
/// `.await` (e.g. creating a torrent) has to happen *before* calling this,
/// never inside it, since a `std::sync::MutexGuard` can't be held across an
/// await point.
///
/// Only for callers that actually mutate; reads belong in [`read_store`].
pub(crate) fn with_store<R>(
    f: impl FnOnce(&mut Vec<Collection>) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    lock_loaded(|collections| {
        let before = collections.len();
        let result = f(collections)?;
        let after = collections.len();
        if before != after {
            clog!("collab_store", "with_store: collection count {before} -> {after}");
        }
        save(collections)?;
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::DeviceIdentity;

    #[test]
    fn persisted_round_trip_preserves_id_collaborators_and_manifest() {
        let identity = DeviceIdentity::generate();
        let mut collection = Collection::new("Iceland 2024".into());
        collection.collaborators.push(Collaborator::new(
            identity.device_id(),
            "Maya".into(),
            Role::Admin,
            1_000,
        ));
        collection.add_manifest_entry(ManifestEntry::new_signed(
            InfoHash::from_bytes([3; 20]),
            "glacier.mp4".into(),
            None,
            &identity,
            2_000,
        ));

        let persisted = to_persisted(&collection);
        let reloaded = from_persisted(&persisted).unwrap();

        assert_eq!(reloaded.id, collection.id);
        assert_eq!(reloaded.name, collection.name);
        assert_eq!(
            reloaded.rendezvous_key().to_hex(),
            collection.rendezvous_key().to_hex()
        );
        assert_eq!(reloaded.collaborators.len(), 1);
        assert_eq!(reloaded.collaborators[0].display_name, "Maya");
        assert!(reloaded.collaborators[0].is_admin());
        assert_eq!(reloaded.manifest().len(), 1);
        assert!(reloaded.manifest().contains(&InfoHash::from_bytes([3; 20])));
    }

    #[test]
    fn save_never_truncates_the_existing_file_before_the_new_one_is_complete() {
        // The failure this guards against actually happened: a plain
        // fs::write opens with O_TRUNC, so a full disk (or a crash) partway
        // through left collections.json empty and every collection was lost.
        // Writing to a sibling temp file and renaming means the real path
        // only ever holds a complete document.
        let dir = std::env::temp_dir().join(format!("portalis-save-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("collections.json");
        std::fs::write(&path, b"{\"collections\":[]}").unwrap();

        // Mirror save()'s write strategy against this temp path.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, b"{\"collections\":[{}]}").unwrap();
        // Before the rename the original is still fully intact — that is the
        // whole property. A truncating write would have emptied it by now.
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{\"collections\":[]}",
            "the live file must stay untouched until the replacement is complete"
        );
        std::fs::rename(&tmp, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"collections\":[{}]}");
        assert!(!tmp.exists(), "rename must consume the temp file");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn persisted_manifest_entries_still_verify_after_round_trip() {
        let identity = DeviceIdentity::generate();
        let mut collection = Collection::new("Band Practice".into());
        collection.add_manifest_entry(ManifestEntry::new_signed(
            InfoHash::from_bytes([5; 20]),
            "take_4.wav".into(),
            None,
            &identity,
            3_000,
        ));

        let reloaded = from_persisted(&to_persisted(&collection)).unwrap();

        assert!(reloaded.manifest().entries().all(|e| e.verify()));
    }
}
