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
    /// Paste-able invite: hex encoding of `<secret hex>:<name>[@addr1,addr2,...]`
    /// (the secret and name are what joining needs; the optional trailing
    /// addresses are *this device's* current sync endpoints — LAN, and
    /// public IP when discoverable — so the joiner can sync immediately
    /// instead of typing an address by hand; hints tied to the moment the
    /// invite was generated, not durable state). The outer hex layer isn't
    /// encryption — the code itself is already the join credential, so
    /// there's no key that could gate it without also gating legitimate
    /// use. It exists so a screenshot or clipboard-history leak doesn't
    /// casually expose your LAN/public IP and collection name in plain
    /// text; decoding is deliberate, not accidental.
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
    native::create_collab_collection(name).await
}

/// Joins a collection from an invite code someone else shared. Adds this
/// device as a `Member` collaborator; the manifest starts empty until a
/// later phase's sync protocol pulls in what other collaborators already
/// added.
pub async fn join_collab_collection(
    invite_code: String,
    display_name: String,
) -> anyhow::Result<CollabCollectionInfo> {
    native::join_collab_collection(invite_code, display_name).await
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
    native::add_media_to_collab_collection(collection_id, label, files).await
}

/// Every collab collection this device knows about (created or joined).
pub async fn list_collab_collections() -> anyhow::Result<Vec<CollabCollectionInfo>> {
    native::list_collab_collections().await
}

/// Starts this device's manifest-sync listener (idempotent) and returns
/// the addresses another device can sync with (comma-separated: LAN, and
/// public IP when discoverable). Phase 2 scaffolding — Phase 3's DHT
/// rendezvous removes the need to ever show or type an address (see
/// `collab_sync.rs`).
pub async fn collab_sync_address() -> anyhow::Result<String> {
    native::collab_sync_address().await
}

/// Starts downloading every media item in this collection over ordinary
/// BitTorrent, handing librqbit the peer addresses learned during sync as
/// direct connection hints (no DHT wait on a LAN). Returns how many items
/// were added.
pub async fn fetch_collab_collection_media(collection_id: String) -> anyhow::Result<u32> {
    native::fetch_collab_collection_media(collection_id).await
}

/// One full manifest sync with the peer at `peer_addr`: exchange signed
/// manifest entries + collaborator lists for this collection and CRDT-merge
/// both ways. Returns the collection's updated state.
pub async fn sync_collab_collection(
    collection_id: String,
    peer_addr: String,
) -> anyhow::Result<CollabCollectionInfo> {
    native::sync_collab_collection(collection_id, peer_addr).await
}

/// Forgets this collab collection on this device — removes it from
/// `collections.json` entirely. Local only: other collaborators (if any
/// synced with this device before) keep their own copies; there's no
/// "delete for everyone" in a grow-only-manifest design, and this doesn't
/// attempt one.
pub async fn delete_collab_collection(collection_id: String) -> anyhow::Result<()> {
    native::delete_collab_collection(collection_id).await
}

mod native {
    use anyhow::Context;
    use crate::collab_store::with_store;
    use crate::domain::collaborator::{Collaborator, Role};
    use crate::domain::collection::{Collection, CollectionId};
    use crate::domain::invite::InviteSecret;
    use crate::domain::manifest::{InfoHash, ManifestEntry};
    use crate::log::clog;

    use super::{CollabCollectionInfo, CollaboratorInfo, ManifestEntryInfo};

    /// Current sync endpoints to embed in invites: LAN always, public IP
    /// when discoverable. Starts the listener as a side effect, so a
    /// device that just *generated* an invite is already reachable.
    async fn current_sync_addresses() -> Vec<String> {
        let Ok(addr) = crate::collab_sync::ensure_listener().await else {
            clog!("collab", "current_sync_addresses: couldn't start the listener, returning no addresses");
            return Vec::new();
        };
        let mut addrs = vec![format!("{}:{}", crate::collab_sync::lan_ip(), addr.port())];
        if let Some(public) = crate::collab_sync::public_ip().await {
            let candidate = format!("{public}:{}", addr.port());
            if !addrs.contains(&candidate) {
                addrs.push(candidate);
            }
        }
        clog!("collab", "current_sync_addresses: {addrs:?}");
        addrs
    }

    fn to_info(collection: &Collection, sync_addrs: &[String]) -> CollabCollectionInfo {
        let plain = if sync_addrs.is_empty() {
            format!("{}:{}", collection.invite_secret_hex(), collection.name)
        } else {
            format!(
                "{}:{}@{}",
                collection.invite_secret_hex(),
                collection.name,
                sync_addrs.join(",")
            )
        };
        // Hex-wrapped, not encrypted (see the field doc on
        // CollabCollectionInfo::invite_code for why encryption wouldn't
        // add anything here) — just opaque enough that a screenshot or
        // clipboard-history leak doesn't casually show your IP/collection
        // name in plain text.
        let invite_code = hex::encode(plain.as_bytes());
        CollabCollectionInfo {
            id: collection.id.to_string(),
            name: collection.name.clone(),
            invite_code,
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
        clog!("collab", "create_collab_collection: name={name:?}");
        let identity = crate::device::current_identity()?;
        let addrs = current_sync_addresses().await;
        let result = with_store(|collections| {
            let mut collection = Collection::new(name);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                "Me".to_string(),
                Role::Admin,
                now_unix_ms(),
            ));
            let info = to_info(&collection, &addrs);
            collections.push(collection);
            Ok(info)
        });
        match &result {
            Ok(info) => clog!(
                "collab",
                "create_collab_collection: created id={} invite_code_len={}",
                info.id,
                info.invite_code.len()
            ),
            Err(e) => clog!("collab", "create_collab_collection: failed: {e:?}"),
        }
        result
    }

    /// Un-hexes the invite code, then splits the resulting
    /// `<secret>:<name>[@addr1,addr2]` into its parts. The address suffix
    /// is only treated as one if every comma-separated piece looks like
    /// `host:port` — a name that merely contains `@` stays a name.
    fn parse_invite_code(invite_code: &str) -> anyhow::Result<(String, String, Vec<String>)> {
        let bytes = hex::decode(invite_code.trim())
            .context("invite code isn't valid — check it was copied in full")?;
        let decoded = String::from_utf8(bytes)
            .context("invite code isn't valid — check it was copied in full")?;
        let (secret_hex, rest) = decoded
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invite code is malformed"))?;
        let parsed = if let Some((name, suffix)) = rest.rsplit_once('@') {
            let addrs: Vec<String> = suffix.split(',').map(str::to_string).collect();
            let all_look_like_addrs = !addrs.is_empty()
                && addrs.iter().all(|a| {
                    a.rsplit_once(':')
                        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
                });
            if all_look_like_addrs {
                (secret_hex.to_string(), name.to_string(), addrs)
            } else {
                (secret_hex.to_string(), rest.to_string(), Vec::new())
            }
        } else {
            (secret_hex.to_string(), rest.to_string(), Vec::new())
        };
        clog!(
            "collab",
            "parse_invite_code: name={:?} addr_count={}",
            parsed.1,
            parsed.2.len()
        );
        Ok(parsed)
    }

    pub(super) async fn join_collab_collection(
        invite_code: String,
        display_name: String,
    ) -> anyhow::Result<CollabCollectionInfo> {
        clog!("collab", "join_collab_collection: invite_code_len={} display_name={display_name:?}", invite_code.len());
        let (secret_hex, name, peer_addrs) = parse_invite_code(&invite_code)?;
        let secret = InviteSecret::from_hex(&secret_hex)?;
        let rendezvous_key_hex = secret.derive_rendezvous_key().to_hex();
        let identity = crate::device::current_identity()?;
        let own_addrs = current_sync_addresses().await;
        clog!(
            "collab",
            "join: name={name:?} peer_addrs={peer_addrs:?} rendezvous_key={}… own_addrs={own_addrs:?}",
            &rendezvous_key_hex[..8.min(rendezvous_key_hex.len())],
        );

        let (id, info) = with_store(|collections| {
            let mut collection = Collection::join(name.to_string(), secret);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                display_name.clone(),
                Role::Member,
                now_unix_ms(),
            ));
            let info = to_info(&collection, &own_addrs);
            let id = collection.id;
            collections.push(collection);
            Ok((id, info))
        })?;
        clog!("collab", "join: local record created, id={id}");

        // Best-effort sync with the inviter, via the addresses embedded in
        // the code — run in the *background*, not awaited here. Either
        // address can take up to ~15s to time out (and there can be two),
        // which made a plain `.await` here look like the app had hung on
        // join; the join itself must stand immediately regardless of
        // whether those addresses turn out to be reachable. Whoever's
        // looking at the collection (User screen, or the manual sync
        // button) picks up the result once/if this finishes — there's
        // deliberately no signal back to the caller of *this* function.
        if peer_addrs.is_empty() {
            clog!("collab", "join: invite carried no addresses, skipping auto-sync");
        } else {
            let rendezvous_key_hex = rendezvous_key_hex.clone();
            tokio::spawn(async move {
                match crate::collab_sync::sync_with_any(&rendezvous_key_hex, &peer_addrs).await {
                    Ok(()) => clog!("collab", "join: background auto-sync succeeded"),
                    Err(e) => clog!("collab", "join: background auto-sync failed: {e:?}"),
                }
            });
        }
        clog!(
            "collab",
            "join_collab_collection: done (returning immediately, sync continues in background), \
             media={} collaborators={}",
            info.media.len(),
            info.collaborators.len()
        );
        Ok(info)
    }

    pub(super) async fn add_media_to_collab_collection(
        collection_id: String,
        label: String,
        files: Vec<crate::torrent::NewFile>,
    ) -> anyhow::Result<CollabCollectionInfo> {
        clog!(
            "collab",
            "add_media_to_collab_collection: collection_id={collection_id} label={label:?} files={}",
            files.len()
        );
        let identity = crate::device::current_identity()?;
        let id = CollectionId::from_string(&collection_id)?;
        let addrs = current_sync_addresses().await;

        // A fresh torrent per batch — its own directory, own info-hash —
        // rather than growing one torrent, since a torrent's piece layout
        // is fixed forever at creation (see the backend README). The
        // random suffix keeps each batch's directory distinct even when
        // two batches share the same user-facing label.
        let batch_dir_name = format!("{label}-{}", uuid::Uuid::new_v4());
        let torrent_info = crate::torrent::create_collection(batch_dir_name, files).await?;
        clog!(
            "collab",
            "add_media_to_collab_collection: seeded torrent info_hash={}",
            torrent_info.info_hash
        );
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
            Ok(to_info(collection, &addrs))
        })
    }

    pub(super) async fn list_collab_collections() -> anyhow::Result<Vec<CollabCollectionInfo>> {
        let addrs = current_sync_addresses().await;
        let result: anyhow::Result<Vec<_>> =
            with_store(|collections| Ok(collections.iter().map(|c| to_info(c, &addrs)).collect()));
        if let Ok(infos) = &result {
            clog!("collab", "list_collab_collections: {} collection(s)", infos.len());
        }
        result
    }

    pub(super) async fn collab_sync_address() -> anyhow::Result<String> {
        let addrs = current_sync_addresses().await;
        anyhow::ensure!(!addrs.is_empty(), "couldn't start the sync listener");
        Ok(addrs.join(","))
    }

    pub(super) async fn sync_collab_collection(
        collection_id: String,
        peer_addr: String,
    ) -> anyhow::Result<CollabCollectionInfo> {
        clog!("collab", "sync_collab_collection: collection_id={collection_id} peer_addr={peer_addr:?}");
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
        // The pasted value may itself be a comma-separated list (it's
        // shown that way on the other device's User screen).
        let peer_addrs: Vec<String> = peer_addr.split(',').map(str::to_string).collect();
        crate::collab_sync::sync_with_any(&rendezvous_key_hex, &peer_addrs).await?;
        let addrs = current_sync_addresses().await;
        with_store(|collections| {
            collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| to_info(c, &addrs))
                .ok_or_else(|| anyhow::anyhow!("collection vanished during sync"))
        })
    }

    pub(super) async fn delete_collab_collection(collection_id: String) -> anyhow::Result<()> {
        clog!("collab", "delete_collab_collection: collection_id={collection_id}");
        let id = CollectionId::from_string(&collection_id)?;
        with_store(|collections| {
            let before = collections.len();
            collections.retain(|c| c.id != id);
            anyhow::ensure!(collections.len() != before, "no such collab collection");
            Ok(())
        })
    }

    pub(super) async fn fetch_collab_collection_media(
        collection_id: String,
    ) -> anyhow::Result<u32> {
        clog!("collab", "fetch_collab_collection_media: collection_id={collection_id}");
        let id = CollectionId::from_string(&collection_id)?;
        let (rendezvous_key_hex, info_hashes) = with_store(|collections| {
            let collection = collections
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow::anyhow!("no such collab collection"))?;
            Ok((
                collection.rendezvous_key().to_hex(),
                collection
                    .manifest()
                    .entries()
                    .map(|e| e.info_hash.to_hex())
                    .collect::<Vec<_>>(),
            ))
        })?;
        let peers = crate::collab_sync::learned_bt_peers(&rendezvous_key_hex);
        clog!(
            "collab",
            "fetch_collab_collection_media: {} media item(s), {} learned peer(s)={peers:?}",
            info_hashes.len(),
            peers.len()
        );
        let mut added = 0u32;
        for info_hash in &info_hashes {
            crate::torrent::add_info_hash_with_peers(info_hash, peers.clone()).await?;
            added += 1;
        }
        Ok(added)
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

            let info = to_info(&collection, &[]);

            assert_eq!(info.id, collection.id.to_string());
            assert_eq!(info.collaborators.len(), 1);
            assert_eq!(info.collaborators[0].display_name, "Theo");
            assert!(!info.collaborators[0].is_admin);
            assert_eq!(info.media.len(), 1);
            assert_eq!(info.media[0].name, "RAW_3000");
            // invite_code must round-trip via the exact parsing
            // join_collab_collection uses.
            let (secret_hex, name, addrs) = parse_invite_code(&info.invite_code).unwrap();
            assert_eq!(secret_hex, collection.invite_secret_hex());
            assert_eq!(name, "Studio Shoot");
            assert!(addrs.is_empty());
        }

        #[test]
        fn invite_code_with_addresses_round_trips() {
            let collection = Collection::new("Iceland 2024".into());
            let addrs = vec!["192.168.1.5:5432".to_string(), "82.10.0.7:5432".to_string()];

            let info = to_info(&collection, &addrs);
            let (secret_hex, name, parsed) = parse_invite_code(&info.invite_code).unwrap();

            assert_eq!(secret_hex, collection.invite_secret_hex());
            assert_eq!(name, "Iceland 2024");
            assert_eq!(parsed, addrs);
        }

        #[test]
        fn invite_code_does_not_expose_the_name_or_addresses_in_plain_text() {
            let collection = Collection::new("Iceland 2024".into());
            let addrs = vec!["192.168.1.5:5432".to_string()];

            let info = to_info(&collection, &addrs);

            // The whole point of the hex wrapper: none of the human-
            // readable metadata should be visible without deliberately
            // decoding it (see the invite_code field doc for why this
            // isn't "encryption" — it's leak-resistance, not access
            // control).
            assert!(!info.invite_code.contains("Iceland"));
            assert!(!info.invite_code.contains("192.168.1.5"));
            assert!(hex::decode(&info.invite_code).is_ok());
        }

        #[test]
        fn a_name_containing_at_is_not_mistaken_for_addresses() {
            let collection = Collection::new("party @ Sam's".into());

            let info = to_info(&collection, &[]);
            let (_, name, addrs) = parse_invite_code(&info.invite_code).unwrap();

            assert_eq!(name, "party @ Sam's");
            assert!(addrs.is_empty());
        }
    }
}

