//! Real, growable, invite-based Collections — the FRB-facing adapter over
//! `domain::collection`/`domain::manifest`/`domain::invite`. See
//! `rust/backend/README.md` for the design, and the phased implementation
//! plan this follows (Phase 0: local persistence + single-device
//! create/join/add-media/list, no networking yet — a collection you
//! `join_collab_collection` on this device stays empty until a later
//! phase's manifest-sync protocol exists to actually exchange entries with
//! a peer). This still exercises the real domain layer end-to-end
//! (signing, verification, the grow-only manifest CRDT), just without a
//! peer on the other end yet.
//!
//! Unconditional DTOs/signatures for the same reason as `torrent.rs` and
//! `device.rs` — FRB's generated glue references `crate::collab::*`
//! regardless of any `#[cfg]` on this module's own declaration.

#[derive(Debug, Clone)]
pub struct CollabCollectionInfo {
    pub id: String,
    pub name: String,
    /// Paste-able invite: encodes both the collection name and its invite
    /// secret, so `join_collab_collection` only needs this one string.
    pub invite_code: String,
    pub collaborators: Vec<CollaboratorInfo>,
    pub media: Vec<ManifestEntryInfo>,
}

#[derive(Debug, Clone)]
pub struct CollaboratorInfo {
    pub device_id: String,
    pub display_name: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct ManifestEntryInfo {
    pub info_hash: String,
    pub name: String,
    pub added_by: String,
    pub added_at_unix_ms: i64,
}

/// Creates a new collab collection and persists it. This device is added
/// as the first collaborator (an admin — see the backend README's still-
/// open moderation-semantics question for why this isn't enforced beyond
/// local display).
pub async fn create_collab_collection(name: String) -> anyhow::Result<CollabCollectionInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::create_collab_collection(name).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        native::unsupported_on_web()
    }
}

/// Joins a collection from an invite code someone else shared. Adds this
/// device as a `Member` collaborator; the manifest starts empty until a
/// later phase's sync protocol pulls in what other collaborators already
/// added.
pub async fn join_collab_collection(
    invite_code: String,
    display_name: String,
) -> anyhow::Result<CollabCollectionInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::join_collab_collection(invite_code, display_name).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (invite_code, display_name);
        native::unsupported_on_web()
    }
}

/// Adds a new batch of local files as one new signed manifest entry —
/// creates its own torrent (own info-hash), self-seeded exactly like
/// `torrent::create_collection`, and appends a manifest entry pointing at
/// it. This is how a Collection grows without ever needing one torrent's
/// info-hash to stand for the *whole* collection's identity.
pub async fn add_media_to_collab_collection(
    collection_id: String,
    label: String,
    files: Vec<crate::torrent::NewFile>,
) -> anyhow::Result<CollabCollectionInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::add_media_to_collab_collection(collection_id, label, files).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (collection_id, label, files);
        native::unsupported_on_web()
    }
}

/// Every collab collection this device knows about (created or joined).
pub async fn list_collab_collections() -> anyhow::Result<Vec<CollabCollectionInfo>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::list_collab_collections().await
    }
    #[cfg(target_arch = "wasm32")]
    {
        native::unsupported_on_web()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
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

    use super::{CollabCollectionInfo, CollaboratorInfo, ManifestEntryInfo};

    #[derive(Serialize, Deserialize)]
    struct PersistedManifestEntry {
        info_hash_hex: String,
        name: String,
        thumbnail_hash_hex: Option<String>,
        added_by_hex: String,
        added_at_unix_ms: i64,
        signature_hex: String,
    }

    #[derive(Serialize, Deserialize)]
    struct PersistedCollaborator {
        device_id_hex: String,
        display_name: String,
        is_admin: bool,
        joined_at_unix_ms: i64,
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

    static STORE: Mutex<Option<Vec<Collection>>> = Mutex::new(None);

    fn store_file() -> PathBuf {
        let base = dirs::config_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir);
        base.join("Portalis").join("collections.json")
    }

    fn to_persisted(collection: &Collection) -> PersistedCollection {
        PersistedCollection {
            id: collection.id.to_string(),
            name: collection.name.clone(),
            invite_secret_hex: collection.invite_secret_hex(),
            collaborators: collection
                .collaborators
                .iter()
                .map(|c| PersistedCollaborator {
                    device_id_hex: c.device_id.to_hex(),
                    display_name: c.display_name.clone(),
                    is_admin: c.is_admin(),
                    joined_at_unix_ms: c.joined_at_unix_ms,
                })
                .collect(),
            manifest: collection
                .manifest()
                .entries()
                .map(|e| PersistedManifestEntry {
                    info_hash_hex: e.info_hash.to_hex(),
                    name: e.name.clone(),
                    thumbnail_hash_hex: e.thumbnail_hash.map(hex::encode),
                    added_by_hex: e.added_by.to_hex(),
                    added_at_unix_ms: e.added_at_unix_ms,
                    signature_hex: hex::encode(e.signature_bytes()),
                })
                .collect(),
        }
    }

    fn from_persisted(persisted: &PersistedCollection) -> anyhow::Result<Collection> {
        let id = CollectionId::from_string(&persisted.id)?;
        let invite_secret = InviteSecret::from_hex(&persisted.invite_secret_hex)?;
        let collaborators = persisted
            .collaborators
            .iter()
            .map(|c| -> anyhow::Result<Collaborator> {
                Ok(Collaborator::new(
                    DeviceId::from_hex(&c.device_id_hex)?,
                    c.display_name.clone(),
                    if c.is_admin { Role::Admin } else { Role::Member },
                    c.joined_at_unix_ms,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut manifest = Manifest::new();
        for e in &persisted.manifest {
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
            manifest.add(ManifestEntry::from_signed_parts(
                InfoHash::from_bytes(info_hash_bytes),
                e.name.clone(),
                thumbnail_hash,
                DeviceId::from_hex(&e.added_by_hex)?,
                e.added_at_unix_ms,
                Signature::from_bytes(&signature_bytes),
            ));
        }
        Ok(Collection::from_parts(
            id,
            persisted.name.clone(),
            invite_secret,
            collaborators,
            manifest,
        ))
    }

    fn load_store() -> anyhow::Result<Vec<Collection>> {
        let path = store_file();
        let Ok(bytes) = std::fs::read(&path) else {
            return Ok(Vec::new());
        };
        let persisted: PersistedStore =
            serde_json::from_slice(&bytes).context("parsing collections.json")?;
        persisted.collections.iter().map(from_persisted).collect()
    }

    fn save_store(collections: &[Collection]) -> anyhow::Result<()> {
        let path = store_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {parent:?}"))?;
        }
        let persisted = PersistedStore {
            collections: collections.iter().map(to_persisted).collect(),
        };
        let bytes = serde_json::to_vec_pretty(&persisted)?;
        std::fs::write(&path, bytes).with_context(|| format!("writing {path:?}"))?;
        Ok(())
    }

    /// Locks the in-memory store (lazily loaded from disk on first use),
    /// runs `f`, then persists. `f` must be synchronous — any `.await`
    /// (e.g. creating a torrent) has to happen *before* calling this, never
    /// inside it, since a `std::sync::MutexGuard` can't be held across an
    /// await point.
    fn with_store<R>(
        f: impl FnOnce(&mut Vec<Collection>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let mut guard = STORE.lock().unwrap();
        if guard.is_none() {
            *guard = Some(load_store()?);
        }
        let collections = guard.as_mut().unwrap();
        let result = f(collections)?;
        save_store(collections)?;
        Ok(result)
    }

    fn to_info(collection: &Collection) -> CollabCollectionInfo {
        CollabCollectionInfo {
            id: collection.id.to_string(),
            name: collection.name.clone(),
            invite_code: format!("{}:{}", collection.invite_secret_hex(), collection.name),
            collaborators: collection
                .collaborators
                .iter()
                .map(|c| CollaboratorInfo {
                    device_id: c.device_id.to_hex(),
                    display_name: c.display_name.clone(),
                    is_admin: c.is_admin(),
                })
                .collect(),
            media: collection
                .manifest()
                .entries()
                .map(|e| ManifestEntryInfo {
                    info_hash: e.info_hash.to_hex(),
                    name: e.name.clone(),
                    added_by: e.added_by.to_hex(),
                    added_at_unix_ms: e.added_at_unix_ms,
                })
                .collect(),
        }
    }

    fn now_unix_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    pub(super) async fn create_collab_collection(
        name: String,
    ) -> anyhow::Result<CollabCollectionInfo> {
        let identity = crate::device::current_identity()?;
        with_store(|collections| {
            let mut collection = Collection::new(name);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                "Me".to_string(),
                Role::Admin,
                now_unix_ms(),
            ));
            let info = to_info(&collection);
            collections.push(collection);
            Ok(info)
        })
    }

    pub(super) async fn join_collab_collection(
        invite_code: String,
        display_name: String,
    ) -> anyhow::Result<CollabCollectionInfo> {
        let (secret_hex, name) = invite_code
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invite code is malformed"))?;
        let secret = InviteSecret::from_hex(secret_hex)?;
        let identity = crate::device::current_identity()?;
        with_store(|collections| {
            let mut collection = Collection::join(name.to_string(), secret);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                display_name.clone(),
                Role::Member,
                now_unix_ms(),
            ));
            let info = to_info(&collection);
            collections.push(collection);
            Ok(info)
        })
    }

    pub(super) async fn add_media_to_collab_collection(
        collection_id: String,
        label: String,
        files: Vec<crate::torrent::NewFile>,
    ) -> anyhow::Result<CollabCollectionInfo> {
        let identity = crate::device::current_identity()?;
        let id = CollectionId::from_string(&collection_id)?;

        // A fresh torrent per batch — its own directory, own info-hash —
        // rather than growing one torrent, since a torrent's piece layout
        // is fixed forever at creation (see the backend README). The
        // random suffix keeps each batch's directory distinct even when
        // two batches share the same user-facing label.
        let batch_dir_name = format!("{label}-{}", uuid::Uuid::new_v4());
        let torrent_info = crate::torrent::create_collection(batch_dir_name, files).await?;
        let info_hash_bytes: [u8; 20] = hex::decode(&torrent_info.info_hash)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("torrent info hash is not 20 bytes"))?;

        with_store(|collections| {
            let collection = collections
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow::anyhow!("no such collab collection"))?;
            let entry = ManifestEntry::new_signed(
                InfoHash::from_bytes(info_hash_bytes),
                label,
                None,
                &identity,
                now_unix_ms(),
            );
            anyhow::ensure!(
                collection.add_manifest_entry(entry),
                "failed to add manifest entry (should never happen for a freshly-signed entry)"
            );
            Ok(to_info(collection))
        })
    }

    pub(super) async fn list_collab_collections() -> anyhow::Result<Vec<CollabCollectionInfo>> {
        with_store(|collections| Ok(collections.iter().map(to_info).collect()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::identity::DeviceIdentity;

        /// `to_persisted`/`from_persisted` round-trip, exercised purely in
        /// memory (no real filesystem/global `STORE` involved, unlike the
        /// `pub(super)` functions above — see `device.rs` for why
        /// filesystem-touching, globally-shared state isn't unit-tested
        /// directly).
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
            assert!(reloaded
                .manifest()
                .contains(&InfoHash::from_bytes([3; 20])));
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

        #[test]
        fn to_info_surfaces_collaborators_and_media() {
            let identity = DeviceIdentity::generate();
            let mut collection = Collection::new("Studio Shoot".into());
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                "Theo".into(),
                Role::Member,
                10,
            ));
            collection.add_manifest_entry(ManifestEntry::new_signed(
                InfoHash::from_bytes([7; 20]),
                "RAW_3000".into(),
                None,
                &identity,
                20,
            ));

            let info = to_info(&collection);

            assert_eq!(info.id, collection.id.to_string());
            assert_eq!(info.collaborators.len(), 1);
            assert_eq!(info.collaborators[0].display_name, "Theo");
            assert!(!info.collaborators[0].is_admin);
            assert_eq!(info.media.len(), 1);
            assert_eq!(info.media[0].name, "RAW_3000");
            // invite_code is "<secret hex>:<name>" — must round-trip via
            // the exact split_once(':') parsing join_collab_collection
            // uses.
            let (secret_hex, name) = info.invite_code.split_once(':').unwrap();
            assert_eq!(secret_hex, collection.invite_secret_hex());
            assert_eq!(name, "Studio Shoot");
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod native {
    pub(super) fn unsupported_on_web<T>() -> anyhow::Result<T> {
        anyhow::bail!(
            "Collaborative collections need real OS sockets, which aren't \
             available on Web. Run this on macOS, Android, iOS, Linux, or \
             Windows instead."
        )
    }
}
