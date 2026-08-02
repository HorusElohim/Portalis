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
    let path = crate::paths::state_dir().join("collections.json");
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

/// Drops the in-memory copy so the next access reloads from disk.
///
/// Only for the store test, which redirects `paths::state_dir` and needs the
/// cache to forget whatever a previous access put there. Production has no
/// reason to: the process owns the file for its lifetime.
#[cfg(test)]
pub(crate) fn forget_cache_for_test() {
    *STORE.lock().unwrap() = None;
}

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

/// Renames this device wherever it appears as a collaborator, across every
/// collection, and persists.
///
/// A collaborator record is a *copy* of the name at the moment the collection
/// was created or joined, so renaming the device identity alone left every
/// existing collection showing the old name forever — and kept broadcasting
/// it to peers on the next sync, since the collaborator list is what gets
/// exchanged. Returns how many records changed.
///
/// Safe to call repeatedly: it checks read-only first and only writes when
/// something actually differs, so polling callers don't rewrite the file.
pub(crate) fn rename_device(device_id: &DeviceId, new_name: &str) -> anyhow::Result<usize> {
    let needs_update = read_store(|collections| {
        Ok(collections.iter().any(|c| {
            c.collaborators
                .iter()
                .any(|x| &x.device_id == device_id && x.display_name != new_name)
        }))
    })?;
    if !needs_update {
        return Ok(0);
    }
    with_store(|collections| {
        let mut renamed = 0;
        for collection in collections.iter_mut() {
            for collaborator in collection.collaborators.iter_mut() {
                if &collaborator.device_id == device_id && collaborator.display_name != new_name {
                    collaborator.display_name = new_name.to_string();
                    renamed += 1;
                }
            }
        }
        clog!("collab_store", "rename_device: updated {renamed} collaborator record(s) to {new_name:?}");
        Ok(renamed)
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
        assert_eq!(
            reloaded.manifest().entries().next().unwrap().info_hash.to_hex(),
            InfoHash::from_bytes([3; 20]).to_hex()
        );
    }

    #[test]
    fn renaming_rewrites_every_record_for_that_device_and_leaves_others_alone() {
        // Exercises the rename against the same in-place mutation
        // rename_device performs, without touching the process-wide store.
        let me = DeviceIdentity::generate().device_id();
        let someone_else = DeviceIdentity::generate().device_id();
        let mut collections = vec![
            Collection::new("Trip".into()),
            Collection::new("Band".into()),
        ];
        for c in collections.iter_mut() {
            c.collaborators.push(Collaborator::new(me, "Me".into(), Role::Admin, 0));
            c.collaborators
                .push(Collaborator::new(someone_else, "Theo".into(), Role::Member, 0));
        }

        let mut renamed = 0;
        for collection in collections.iter_mut() {
            for collaborator in collection.collaborators.iter_mut() {
                if collaborator.device_id == me && collaborator.display_name != "Maya" {
                    collaborator.display_name = "Maya".to_string();
                    renamed += 1;
                }
            }
        }

        // Every collection, not just the most recent one.
        assert_eq!(renamed, 2);
        for c in &collections {
            assert_eq!(c.collaborators[0].display_name, "Maya");
            // Renaming this device must not touch anyone else's record.
            assert_eq!(c.collaborators[1].display_name, "Theo");
        }
    }

    /// The store's whole lifecycle, against a real file.
    ///
    /// One test rather than several because the store is process-wide: two
    /// tests exercising it would interleave. Everything it asserts was
    /// previously unreachable — the old version of this test mirrored
    /// `save`'s strategy against a temp path instead of calling `save`, so it
    /// proved a property of `rename` and nothing about this module.
    #[test]
    fn the_store_round_trips_through_a_real_file_and_never_truncates_it() {
        let temp = crate::paths::redirect_to_temp();
        forget_cache_for_test();
        let identity = DeviceIdentity::generate();
        let path = temp.path("collections.json");

        // A read of an empty store must not create the file. `read_store`
        // exists precisely because every access used to write one.
        read_store(|collections| Ok(assert!(collections.is_empty()))).unwrap();
        assert!(!path.exists(), "reading must never write");

        with_store(|collections| Ok(collections.push(seeded(&identity, "Iceland")))).unwrap();
        assert!(path.exists());

        // Reload from disk, not from the cache: this is the restart path.
        forget_cache_for_test();
        read_store(|collections| {
            assert_eq!(collections.len(), 1);
            assert_eq!(collections[0].name, "Iceland");
            // The entry survived with a signature that still verifies, which
            // is what makes it acceptable to a peer.
            assert_eq!(collections[0].manifest().len(), 1);
            Ok(())
        })
        .unwrap();

        // Renaming this device rewrites its collaborator record everywhere,
        // and is a no-op — including no write — when the name already matches.
        assert_eq!(rename_device(&identity.device_id(), "Maya").unwrap(), 1);
        assert_eq!(rename_device(&identity.device_id(), "Maya").unwrap(), 0);
        forget_cache_for_test();
        read_store(|c| Ok(assert_eq!(c[0].collaborators[0].display_name, "Maya"))).unwrap();

        // And the file is only ever replaced whole: a truncating write would
        // have left this empty at some point, which is how a full disk
        // destroyed the real store once.
        assert!(!path.with_extension("json.tmp").exists());
        assert!(serde_json::from_slice::<PersistedStore>(&std::fs::read(&path).unwrap()).is_ok());
    }

    /// The property a full disk destroyed once, asserted against real code.
    ///
    /// A directory nothing can create files in is the same shape as a full
    /// one: the sibling temp file cannot be written, so the save fails — and
    /// the point is that it fails having left the previous document whole. A
    /// truncating `fs::write` opens the destination itself, which empties it
    /// before discovering it cannot finish.
    #[cfg(unix)]
    #[test]
    fn a_write_that_cannot_complete_leaves_the_previous_store_intact() {
        use std::os::unix::fs::PermissionsExt;
        let temp = crate::paths::redirect_to_temp();
        forget_cache_for_test();
        let identity = DeviceIdentity::generate();
        with_store(|c| Ok(c.push(seeded(&identity, "Iceland")))).unwrap();
        let path = temp.path("collections.json");
        let intact = std::fs::read(&path).unwrap();
        let dir = path.parent().unwrap();

        std::fs::set_permissions(dir, PermissionsExt::from_mode(0o500)).unwrap();
        let result = with_store(|c| Ok(c.push(seeded(&identity, "Second"))));
        std::fs::set_permissions(dir, PermissionsExt::from_mode(0o700)).unwrap();

        assert!(result.is_err(), "an unwritable store must report, not swallow");
        assert_eq!(std::fs::read(&path).unwrap(), intact);
        // Worth being explicit: the in-memory copy did take the push, so the
        // cache and the file now disagree until the next reload. That is
        // today's behaviour, not an endorsement of it.
    }

    /// A collection with one signed entry and this device as its collaborator.
    fn seeded(identity: &DeviceIdentity, name: &str) -> Collection {
        let mut collection = Collection::new(name.into());
        collection.collaborators.push(Collaborator::new(
            identity.device_id(),
            "Me".into(),
            Role::Admin,
            1,
        ));
        collection.add_manifest_entry(ManifestEntry::new_signed(
            InfoHash::from_bytes([4; 20]),
            "batch".into(),
            None,
            identity,
            2,
        ));
        collection
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
