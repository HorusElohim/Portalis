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

/// Starts this device's manifest-sync listener (idempotent) and returns
/// the `ip:port` another device on the same network can sync with. Phase 2
/// scaffolding — Phase 3's DHT rendezvous removes the need to ever show or
/// type an address (see `collab_sync.rs`).
pub async fn collab_sync_address() -> anyhow::Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::collab_sync_address().await
    }
    #[cfg(target_arch = "wasm32")]
    {
        native::unsupported_on_web()
    }
}

/// One full manifest sync with the peer at `peer_addr`: exchange signed
/// manifest entries + collaborator lists for this collection and CRDT-merge
/// both ways. Returns the collection's updated state.
pub async fn sync_collab_collection(
    collection_id: String,
    peer_addr: String,
) -> anyhow::Result<CollabCollectionInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::sync_collab_collection(collection_id, peer_addr).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (collection_id, peer_addr);
        native::unsupported_on_web()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::collab_store::with_store;
    use crate::domain::collaborator::{Collaborator, Role};
    use crate::domain::collection::{Collection, CollectionId};
    use crate::domain::invite::InviteSecret;
    use crate::domain::manifest::{InfoHash, ManifestEntry};

    use super::{CollabCollectionInfo, CollaboratorInfo, ManifestEntryInfo};

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

    pub(super) async fn collab_sync_address() -> anyhow::Result<String> {
        let addr = crate::collab_sync::ensure_listener().await?;
        Ok(format!("{}:{}", crate::collab_sync::lan_ip(), addr.port()))
    }

    pub(super) async fn sync_collab_collection(
        collection_id: String,
        peer_addr: String,
    ) -> anyhow::Result<CollabCollectionInfo> {
        let id = CollectionId::from_string(&collection_id)?;
        let rendezvous_key_hex = with_store(|collections| {
            collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.rendezvous_key().to_hex())
                .ok_or_else(|| anyhow::anyhow!("no such collab collection"))
        })?;
        // Make sure our own listener is up before reaching out, so the
        // peer's user can immediately sync back the other way too.
        let _ = crate::collab_sync::ensure_listener().await?;
        crate::collab_sync::sync_with(&rendezvous_key_hex, &peer_addr).await?;
        with_store(|collections| {
            collections
                .iter()
                .find(|c| c.id == id)
                .map(to_info)
                .ok_or_else(|| anyhow::anyhow!("collection vanished during sync"))
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::identity::DeviceIdentity;

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
