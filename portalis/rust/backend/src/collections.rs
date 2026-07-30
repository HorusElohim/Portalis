//! **The** Collections API — the single collection model the whole app
//! renders, and the only collection-shaped module bridged to Flutter.
//!
//! This replaces the two parallel models the app used to carry: `collab.rs`'s
//! invite-based collections (persisted in `collections.json`, shown only on
//! the User screen) and `torrent.rs`'s bare torrents (shown on Home). They
//! were never joined, so a collection you created or joined simply never
//! appeared in the collection list. Phase 4 of the plan in
//! `rust/backend/README.md` is exactly this unification, and this module is
//! where it happens.
//!
//! The join has to live in Rust because this is the only layer that can see
//! *both* sources at once: the persisted manifest (via `collab_store`) and
//! the live BitTorrent session (via `torrent`). Doing it in Dart would mean
//! shipping both halves over FFI and correlating info-hashes in the UI layer.
//!
//! The model:
//!
//! - A [`CollectionInfo`] of kind [`CollectionKind::Shared`] is a real
//!   invite-based collection. Its media is the **union of the files across
//!   every manifest entry's torrent** — which is the whole point of the
//!   design: a collection grows by gaining entries, while each individual
//!   torrent stays immutable.
//! - A manifest entry whose torrent isn't in the local session yet still
//!   appears, as a single [`MediaInfo`] with `fetched: false`. That's the
//!   "known but not downloaded" state — you know it exists because a peer
//!   signed it into the manifest, you just haven't pulled the bytes.
//! - A torrent that no manifest entry claims (added via a magnet link)
//!   surfaces as its own [`CollectionKind::Torrent`] collection, so the
//!   plain-torrent flow keeps working through the same one list.
//!
//! Not FRB-scanned: `collab_store`, `collab_sync`, `domain`, `log` — see
//! `collab_store.rs`'s module doc for why (FRB's codegen scans a listed
//! module's subtree textually, ignoring Rust visibility).

/// What kind of collection this is — the two ways one can come into
/// existence. Both render identically; they differ in what you can *do*
/// with them (only `Shared` has an invite code and can grow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionKind {
    /// Invite-based and growable: has an invite code, collaborators, and a
    /// signed manifest that gains entries over time.
    Shared,
    /// A single plain torrent added by magnet link — no invite, no
    /// collaborators, fixed contents.
    Torrent,
}

#[derive(Debug, Clone)]
pub struct CollectionInfo {
    /// A shared collection's `CollectionId`, or a torrent's info-hash.
    /// Opaque to Flutter; pass it back to the functions below.
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    /// Paste-able invite code — `Shared` collections only. See
    /// [`invite_code_for`] for the format and why it's hex-wrapped.
    pub invite_code: Option<String>,
    pub collaborators: Vec<CollaboratorInfo>,
    /// Every file across every fetched manifest entry, plus one placeholder
    /// per not-yet-fetched entry.
    pub media: Vec<MediaInfo>,
    /// 0.0..=1.0 across all *fetched* bytes. Entries that haven't been
    /// fetched contribute nothing to either side of the ratio — they're
    /// counted by [`pending_media`](Self::pending_media) instead, since
    /// their size isn't knowable until the torrent's metadata arrives.
    pub progress: f64,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    /// Currently-connected peers, summed across this collection's torrents.
    pub live_peers: u32,
    /// Manifest entries with no local torrent yet — "known but not fetched".
    pub pending_media: u32,
    /// Coarse status for display: `seeding` / `downloading` / `pending` /
    /// `empty`. Derived here rather than in the UI so both kinds of
    /// collection describe themselves the same way.
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct CollaboratorInfo {
    pub device_id: String,
    pub display_name: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub name: String,
    /// The info-hash of the torrent (i.e. the manifest entry) this file
    /// belongs to — several files can share one.
    pub info_hash: String,
    /// Real on-disk location, once the file is complete. `None` while
    /// downloading, and always `None` for a not-yet-fetched entry.
    pub absolute_path: Option<String>,
    pub length_bytes: u64,
    pub downloaded_bytes: u64,
    pub progress: f64,
    /// `false` when this stands for a whole manifest entry whose torrent
    /// isn't in the session yet — tap to fetch.
    pub fetched: bool,
    /// Device id of the collaborator who added it (`Shared` only).
    pub added_by: Option<String>,
}

/// Every collection this device knows about, from both sources, joined.
pub async fn list_collections() -> anyhow::Result<Vec<CollectionInfo>> {
    native::list_collections().await
}

/// Creates a new shared collection (empty) and persists it. This device
/// becomes its first collaborator, as admin.
pub async fn create_collection(name: String) -> anyhow::Result<CollectionInfo> {
    native::create_collection(name).await
}

/// Creates a shared collection *and* seeds `files` into it as its first
/// manifest entry — the "share something" flow, which previously produced a
/// bare torrent with no invite code and therefore nothing to share.
pub async fn create_collection_with_media(
    name: String,
    files: Vec<crate::torrent::NewFile>,
) -> anyhow::Result<CollectionInfo> {
    native::create_collection_with_media(name, files).await
}

/// Joins a shared collection from an invite code. Returns immediately; the
/// first sync with the inviter runs in the background (see the note in
/// [`native::join_collection`]).
pub async fn join_collection(
    invite_code: String,
    display_name: String,
) -> anyhow::Result<CollectionInfo> {
    native::join_collection(invite_code, display_name).await
}

/// Adds local files to a shared collection as one new signed manifest entry
/// with its own torrent. This is how a collection grows.
pub async fn add_media_to_collection(
    collection_id: String,
    label: String,
    files: Vec<crate::torrent::NewFile>,
) -> anyhow::Result<CollectionInfo> {
    native::add_media_to_collection(collection_id, label, files).await
}

/// Starts downloading every not-yet-fetched manifest entry, handing librqbit
/// the peer addresses learned during sync as direct connection hints.
/// Returns how many entries were started.
pub async fn fetch_collection_media(collection_id: String) -> anyhow::Result<u32> {
    native::fetch_collection_media(collection_id).await
}

/// One full manifest sync with a peer, for a shared collection.
pub async fn sync_collection(
    collection_id: String,
    peer_addr: String,
) -> anyhow::Result<CollectionInfo> {
    native::sync_collection(collection_id, peer_addr).await
}

/// Forgets a collection on this device. For a `Shared` one that means
/// dropping it from `collections.json` (other collaborators keep their
/// copies — there's no "delete for everyone" in a grow-only design); for a
/// `Torrent` one it means removing it from the session. Downloaded files are
/// left on disk either way.
pub async fn delete_collection(collection_id: String) -> anyhow::Result<()> {
    native::delete_collection(collection_id).await
}

/// This device's manifest-sync endpoints (LAN, plus public IP when
/// discoverable), comma-separated. Starts the sync listener as a side
/// effect, so calling it makes this device reachable.
pub async fn sync_address() -> anyhow::Result<String> {
    native::sync_address().await
}

mod native {
    use std::collections::{HashMap, HashSet};

    use crate::collab_store::with_store;
    use crate::domain::collaborator::{Collaborator, Role};
    use crate::domain::collection::{Collection, CollectionId};
    use crate::domain::invite::InviteSecret;
    use crate::domain::manifest::{InfoHash, ManifestEntry};
    use crate::log::clog;
    use crate::torrent::TorrentInfo;

    use anyhow::Context;

    use super::{CollaboratorInfo, CollectionInfo, CollectionKind, MediaInfo};

    fn now_unix_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    /// Current sync endpoints to embed in invites: LAN always, public IP
    /// when discoverable. Starts the listener as a side effect, so a device
    /// that just generated an invite is already reachable.
    async fn current_sync_addresses() -> Vec<String> {
        let Ok(addr) = crate::collab_sync::ensure_listener().await else {
            clog!("collections", "current_sync_addresses: listener wouldn't start, no addresses");
            return Vec::new();
        };
        // Every real interface address, not just the default route's — a VPN
        // owning the default route would otherwise hide the Wi-Fi address the
        // peer can actually reach. See collab_sync::lan_ips.
        let mut addrs: Vec<String> = crate::collab_sync::lan_ips()
            .into_iter()
            .map(|ip| format!("{ip}:{}", addr.port()))
            .collect();
        if let Some(public) = crate::collab_sync::public_ip().await {
            let candidate = format!("{public}:{}", addr.port());
            if !addrs.contains(&candidate) {
                addrs.push(candidate);
            }
        }
        clog!("collections", "current_sync_addresses: {addrs:?}");
        addrs
    }

    /// Invite format: hex of `<secret hex>:<name>[@addr1,addr2,...]`. The
    /// outer hex isn't encryption — the code *is* the join credential, so
    /// there's no key that could gate it without also gating legitimate use.
    /// It's there so a screenshot or clipboard-history leak doesn't casually
    /// expose your LAN/public IP and collection name in plain text.
    fn invite_code_for(collection: &Collection, sync_addrs: &[String]) -> String {
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
        hex::encode(plain.as_bytes())
    }

    /// Un-hexes an invite code and splits it into `(secret, name, addresses)`.
    /// The address suffix is only treated as one if every comma-separated
    /// piece looks like `host:port` — a name that merely contains `@` stays
    /// part of the name.
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
            "collections",
            "parse_invite_code: name={:?} addrs={:?}",
            parsed.1,
            parsed.2
        );
        Ok(parsed)
    }

    fn collaborators_of(collection: &Collection) -> Vec<CollaboratorInfo> {
        collection
            .collaborators
            .iter()
            .map(|c| CollaboratorInfo {
                device_id: c.device_id.to_hex(),
                display_name: c.display_name.clone(),
                is_admin: c.is_admin(),
            })
            .collect()
    }

    /// Info-hashes are compared case-insensitively throughout: librqbit
    /// renders them via `Id20::as_string` and our manifest via `hex::encode`,
    /// and nothing guarantees those agree on case forever.
    fn norm(info_hash: &str) -> String {
        info_hash.to_lowercase()
    }

    fn media_from_torrent(
        torrent: &TorrentInfo,
        added_by: Option<String>,
    ) -> Vec<MediaInfo> {
        torrent
            .files
            .iter()
            .map(|f| {
                let progress = if f.length_bytes > 0 {
                    (f.downloaded_bytes as f64 / f.length_bytes as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                MediaInfo {
                    name: f.name.clone(),
                    info_hash: norm(&torrent.info_hash),
                    // Only expose a path once the file is actually complete —
                    // a partially-written file will not open or decode.
                    absolute_path: (progress >= 1.0).then(|| f.absolute_path.clone()),
                    length_bytes: f.length_bytes,
                    downloaded_bytes: f.downloaded_bytes,
                    progress,
                    fetched: true,
                    added_by: added_by.clone(),
                }
            })
            .collect()
    }

    /// Rolls per-torrent stats up into one collection-level summary.
    struct Totals {
        total_bytes: u64,
        downloaded_bytes: u64,
        uploaded_bytes: u64,
        download_mbps: f64,
        upload_mbps: f64,
        live_peers: u32,
    }

    impl Totals {
        fn new() -> Self {
            Self {
                total_bytes: 0,
                downloaded_bytes: 0,
                uploaded_bytes: 0,
                download_mbps: 0.0,
                upload_mbps: 0.0,
                live_peers: 0,
            }
        }

        fn add(&mut self, t: &TorrentInfo) {
            self.total_bytes += t.total_bytes;
            self.downloaded_bytes += t.progress_bytes;
            self.uploaded_bytes += t.uploaded_bytes;
            self.download_mbps += t.download_mbps;
            self.upload_mbps += t.upload_mbps;
            // Summed, not deduplicated: the same peer serving two of a
            // collection's torrents genuinely is two live connections, which
            // is what this number describes.
            self.live_peers += t.live_peers;
        }

        fn progress(&self) -> f64 {
            if self.total_bytes == 0 {
                0.0
            } else {
                (self.downloaded_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
            }
        }
    }

    fn state_for(totals: &Totals, pending_media: u32, media_len: usize) -> String {
        if media_len == 0 {
            "empty".to_string()
        } else if pending_media > 0 && totals.total_bytes == 0 {
            "pending".to_string()
        } else if totals.progress() >= 1.0 && pending_media == 0 {
            "seeding".to_string()
        } else {
            "downloading".to_string()
        }
    }

    pub(super) async fn list_collections() -> anyhow::Result<Vec<CollectionInfo>> {
        // Both halves of the join are gathered *before* taking the store
        // lock — `with_store`'s closure is synchronous and can't await.
        let torrents = crate::torrent::list_torrents().await.unwrap_or_else(|e| {
            clog!("collections", "list_collections: torrent session unavailable ({e:#}) — \
                 shared collections still list, with everything marked not-fetched");
            Vec::new()
        });
        let addrs = current_sync_addresses().await;

        let by_hash: HashMap<String, &TorrentInfo> = torrents
            .iter()
            .map(|t| (norm(&t.info_hash), t))
            .collect();

        let (mut result, claimed) = with_store(|collections| {
            let mut claimed: HashSet<String> = HashSet::new();
            let mut out = Vec::with_capacity(collections.len());
            for collection in collections.iter() {
                let mut totals = Totals::new();
                let mut media = Vec::new();
                let mut pending_media = 0u32;

                for entry in collection.manifest().entries() {
                    let hash = norm(&entry.info_hash.to_hex());
                    claimed.insert(hash.clone());
                    match by_hash.get(&hash) {
                        Some(torrent) => {
                            totals.add(torrent);
                            media.extend(media_from_torrent(
                                torrent,
                                Some(entry.added_by.to_hex()),
                            ));
                        }
                        None => {
                            // Known but not fetched: one placeholder standing
                            // for the whole entry. Its real file list isn't
                            // knowable until the torrent's metadata arrives.
                            pending_media += 1;
                            media.push(MediaInfo {
                                name: entry.name.clone(),
                                info_hash: hash,
                                absolute_path: None,
                                length_bytes: 0,
                                downloaded_bytes: 0,
                                progress: 0.0,
                                fetched: false,
                                added_by: Some(entry.added_by.to_hex()),
                            });
                        }
                    }
                }

                out.push(CollectionInfo {
                    id: collection.id.to_string(),
                    name: collection.name.clone(),
                    kind: CollectionKind::Shared,
                    invite_code: Some(invite_code_for(collection, &addrs)),
                    collaborators: collaborators_of(collection),
                    progress: totals.progress(),
                    total_bytes: totals.total_bytes,
                    downloaded_bytes: totals.downloaded_bytes,
                    uploaded_bytes: totals.uploaded_bytes,
                    download_mbps: totals.download_mbps,
                    upload_mbps: totals.upload_mbps,
                    live_peers: totals.live_peers,
                    state: state_for(&totals, pending_media, media.len()),
                    pending_media,
                    media,
                });
            }
            Ok((out, claimed))
        })?;

        // Anything the session holds that no manifest claims is a plain
        // torrent the user added directly — its own collection of one.
        for torrent in &torrents {
            if claimed.contains(&norm(&torrent.info_hash)) {
                continue;
            }
            let mut totals = Totals::new();
            totals.add(torrent);
            let media = media_from_torrent(torrent, None);
            result.push(CollectionInfo {
                id: norm(&torrent.info_hash),
                name: torrent.name.clone(),
                kind: CollectionKind::Torrent,
                invite_code: None,
                collaborators: Vec::new(),
                progress: totals.progress(),
                total_bytes: totals.total_bytes,
                downloaded_bytes: totals.downloaded_bytes,
                uploaded_bytes: totals.uploaded_bytes,
                download_mbps: totals.download_mbps,
                upload_mbps: totals.upload_mbps,
                live_peers: totals.live_peers,
                state: state_for(&totals, 0, media.len()),
                pending_media: 0,
                media,
            });
        }

        clog!(
            "collections",
            "list_collections: {} collection(s) ({} shared, {} plain torrent)",
            result.len(),
            result.iter().filter(|c| c.kind == CollectionKind::Shared).count(),
            result.iter().filter(|c| c.kind == CollectionKind::Torrent).count(),
        );
        Ok(result)
    }

    /// Re-reads one collection through the same join `list_collections` uses,
    /// so every command returns exactly the shape the list does rather than a
    /// hand-built near-copy that could drift.
    async fn reload(collection_id: &str) -> anyhow::Result<CollectionInfo> {
        list_collections()
            .await?
            .into_iter()
            .find(|c| c.id == collection_id)
            .ok_or_else(|| anyhow::anyhow!("no such collection"))
    }

    pub(super) async fn create_collection(name: String) -> anyhow::Result<CollectionInfo> {
        clog!("collections", "create_collection: name={name:?}");
        let identity = crate::device::current_identity()?;
        let id = with_store(|collections| {
            let mut collection = Collection::new(name);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                "Me".to_string(),
                Role::Admin,
                now_unix_ms(),
            ));
            let id = collection.id;
            collections.push(collection);
            Ok(id)
        })?;
        clog!("collections", "create_collection: created id={id}");
        reload(&id.to_string()).await
    }

    pub(super) async fn create_collection_with_media(
        name: String,
        files: Vec<crate::torrent::NewFile>,
    ) -> anyhow::Result<CollectionInfo> {
        clog!(
            "collections",
            "create_collection_with_media: name={name:?} files={}",
            files.len()
        );
        let created = create_collection(name.clone()).await?;
        add_media_to_collection(created.id, name, files).await
    }

    pub(super) async fn join_collection(
        invite_code: String,
        display_name: String,
    ) -> anyhow::Result<CollectionInfo> {
        clog!(
            "collections",
            "join_collection: invite_code_len={} display_name={display_name:?}",
            invite_code.len()
        );
        let (secret_hex, name, peer_addrs) = parse_invite_code(&invite_code)?;
        let secret = InviteSecret::from_hex(&secret_hex)?;
        let rendezvous_key_hex = secret.derive_rendezvous_key().to_hex();
        let identity = crate::device::current_identity()?;

        let id = with_store(|collections| {
            let mut collection = Collection::join(name.to_string(), secret);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                display_name.clone(),
                Role::Member,
                now_unix_ms(),
            ));
            let id = collection.id;
            collections.push(collection);
            Ok(id)
        })?;
        clog!("collections", "join_collection: local record created, id={id}");

        // Best-effort first sync with the inviter, in the *background*. Each
        // candidate address can take up to the sync IO timeout to fail, and
        // an invite can carry two — awaiting that here made joining look like
        // the app had hung. The local record stands regardless of whether
        // those addresses turn out to be reachable; whoever is looking at the
        // collection picks up the result once/if the sync lands.
        if peer_addrs.is_empty() {
            clog!("collections", "join_collection: invite carried no addresses, no auto-sync");
        } else {
            tokio::spawn(async move {
                match crate::collab_sync::sync_with_any(&rendezvous_key_hex, &peer_addrs).await {
                    Ok(()) => clog!("collections", "join_collection: background auto-sync succeeded"),
                    Err(e) => clog!("collections", "join_collection: background auto-sync failed: {e:?}"),
                }
            });
        }
        reload(&id.to_string()).await
    }

    pub(super) async fn add_media_to_collection(
        collection_id: String,
        label: String,
        files: Vec<crate::torrent::NewFile>,
    ) -> anyhow::Result<CollectionInfo> {
        clog!(
            "collections",
            "add_media_to_collection: id={collection_id} label={label:?} files={}",
            files.len()
        );
        let identity = crate::device::current_identity()?;
        let id = CollectionId::from_string(&collection_id)?;

        // A fresh torrent per batch — its own directory, own info-hash —
        // since a torrent's piece layout is fixed forever at creation (see
        // the backend README). The random suffix keeps each batch's directory
        // distinct even when two batches share a label.
        let batch_dir_name = format!("{label}-{}", uuid::Uuid::new_v4());
        let torrent_info = crate::torrent::create_collection(batch_dir_name, files).await?;
        clog!(
            "collections",
            "add_media_to_collection: seeded torrent info_hash={}",
            torrent_info.info_hash
        );
        let info_hash_bytes: [u8; 20] = hex::decode(&torrent_info.info_hash)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("torrent info hash is not 20 bytes"))?;

        with_store(|collections| {
            let collection = collections
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow::anyhow!("no such collection"))?;
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
            Ok(())
        })?;
        reload(&collection_id).await
    }

    pub(super) async fn fetch_collection_media(collection_id: String) -> anyhow::Result<u32> {
        clog!("collections", "fetch_collection_media: id={collection_id}");
        let id = CollectionId::from_string(&collection_id)?;
        let (rendezvous_key_hex, info_hashes) = with_store(|collections| {
            let collection = collections
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow::anyhow!("no such collection"))?;
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
            "collections",
            "fetch_collection_media: {} entries, {} learned peer(s)={peers:?}",
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

    pub(super) async fn sync_collection(
        collection_id: String,
        peer_addr: String,
    ) -> anyhow::Result<CollectionInfo> {
        clog!("collections", "sync_collection: id={collection_id} peer_addr={peer_addr:?}");
        let id = CollectionId::from_string(&collection_id)?;
        let rendezvous_key_hex = with_store(|collections| {
            collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.rendezvous_key().to_hex())
                .ok_or_else(|| anyhow::anyhow!("no such collection"))
        })?;
        // Make sure our own listener is up before reaching out, so the peer
        // can immediately sync back the other way too.
        let _ = crate::collab_sync::ensure_listener().await?;
        // The pasted value may itself be a comma-separated list — that's how
        // a sync address is displayed on the other device.
        let peer_addrs: Vec<String> = peer_addr.split(',').map(str::to_string).collect();
        crate::collab_sync::sync_with_any(&rendezvous_key_hex, &peer_addrs).await?;
        reload(&collection_id).await
    }

    pub(super) async fn delete_collection(collection_id: String) -> anyhow::Result<()> {
        clog!("collections", "delete_collection: id={collection_id}");
        // A shared collection's id is a UUID; a plain torrent's is its
        // info-hash. Which one it parses as tells us where it lives.
        if let Ok(id) = CollectionId::from_string(&collection_id) {
            return with_store(|collections| {
                let before = collections.len();
                collections.retain(|c| c.id != id);
                anyhow::ensure!(collections.len() != before, "no such collection");
                Ok(())
            });
        }
        crate::torrent::forget_torrent(&collection_id).await
    }

    pub(super) async fn sync_address() -> anyhow::Result<String> {
        let addrs = current_sync_addresses().await;
        anyhow::ensure!(!addrs.is_empty(), "couldn't start the sync listener");
        Ok(addrs.join(","))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::identity::DeviceIdentity;

        #[test]
        fn invite_code_round_trips_through_the_exact_parser_join_uses() {
            let collection = Collection::new("Studio Shoot".into());

            let code = invite_code_for(&collection, &[]);
            let (secret_hex, name, addrs) = parse_invite_code(&code).unwrap();

            assert_eq!(secret_hex, collection.invite_secret_hex());
            assert_eq!(name, "Studio Shoot");
            assert!(addrs.is_empty());
        }

        #[test]
        fn invite_code_with_addresses_round_trips() {
            let collection = Collection::new("Iceland 2024".into());
            let addrs = vec!["192.168.1.5:5432".to_string(), "82.10.0.7:5432".to_string()];

            let code = invite_code_for(&collection, &addrs);
            let (secret_hex, name, parsed) = parse_invite_code(&code).unwrap();

            assert_eq!(secret_hex, collection.invite_secret_hex());
            assert_eq!(name, "Iceland 2024");
            assert_eq!(parsed, addrs);
        }

        #[test]
        fn a_name_containing_at_is_not_mistaken_for_addresses() {
            let collection = Collection::new("party @ Sam's".into());

            let (_, name, addrs) = parse_invite_code(&invite_code_for(&collection, &[])).unwrap();

            assert_eq!(name, "party @ Sam's");
            assert!(addrs.is_empty());
        }

        #[test]
        fn invite_code_does_not_expose_the_name_or_addresses_in_plain_text() {
            let collection = Collection::new("Iceland 2024".into());

            let code = invite_code_for(&collection, &["192.168.1.5:5432".to_string()]);

            // The whole point of the hex wrapper — see invite_code_for.
            assert!(!code.contains("Iceland"));
            assert!(!code.contains("192.168.1.5"));
            assert!(hex::decode(&code).is_ok());
        }

        fn torrent(info_hash: &str, files: Vec<(&str, u64, u64)>) -> TorrentInfo {
            let total: u64 = files.iter().map(|(_, len, _)| len).sum();
            let done: u64 = files.iter().map(|(_, _, d)| d).sum();
            TorrentInfo {
                id: 1,
                info_hash: info_hash.to_string(),
                name: "batch".into(),
                state: "live".into(),
                progress_bytes: done,
                total_bytes: total,
                uploaded_bytes: 0,
                download_mbps: 0.0,
                upload_mbps: 0.0,
                finished: done >= total,
                error: None,
                live_peers: 2,
                files: files
                    .into_iter()
                    .map(|(name, len, downloaded)| crate::torrent::TorrentFile {
                        name: name.to_string(),
                        absolute_path: format!("/tmp/{name}"),
                        length_bytes: len,
                        downloaded_bytes: downloaded,
                    })
                    .collect(),
            }
        }

        #[test]
        fn a_torrents_files_all_become_media_of_the_owning_collection() {
            let t = torrent("aa", vec![("a.mp4", 100, 100), ("b.mp4", 100, 50)]);

            let media = media_from_torrent(&t, Some("device".into()));

            assert_eq!(media.len(), 2);
            assert!(media.iter().all(|m| m.fetched));
            // Only the complete file exposes a path — a half-written file
            // won't open.
            assert_eq!(media[0].absolute_path.as_deref(), Some("/tmp/a.mp4"));
            assert_eq!(media[1].absolute_path, None);
            assert_eq!(media[1].progress, 0.5);
        }

        #[test]
        fn totals_sum_across_every_torrent_in_a_collection() {
            let mut totals = Totals::new();
            totals.add(&torrent("aa", vec![("a", 100, 100)]));
            totals.add(&torrent("bb", vec![("b", 300, 100)]));

            assert_eq!(totals.total_bytes, 400);
            assert_eq!(totals.downloaded_bytes, 200);
            assert_eq!(totals.live_peers, 4);
            assert_eq!(totals.progress(), 0.5);
        }

        #[test]
        fn state_reflects_pending_entries_not_just_byte_progress() {
            let mut complete = Totals::new();
            complete.add(&torrent("aa", vec![("a", 100, 100)]));

            // Fully downloaded, but another entry hasn't been fetched at all
            // — that's still "downloading", not "seeding".
            assert_eq!(state_for(&complete, 1, 2), "downloading");
            assert_eq!(state_for(&complete, 0, 1), "seeding");
            assert_eq!(state_for(&Totals::new(), 3, 3), "pending");
            assert_eq!(state_for(&Totals::new(), 0, 0), "empty");
        }

        #[test]
        fn info_hashes_match_regardless_of_case() {
            // librqbit and our manifest produce hex independently; nothing
            // guarantees they agree on case, and a mismatch would silently
            // orphan every fetched entry.
            assert_eq!(norm("AABBCC"), norm("aabbcc"));
        }

        #[test]
        fn signed_manifest_entries_carry_their_author_into_media() {
            let identity = DeviceIdentity::generate();
            let entry = ManifestEntry::new_signed(
                InfoHash::from_bytes([7; 20]),
                "RAW_3000".into(),
                None,
                &identity,
                20,
            );

            let media = media_from_torrent(
                &torrent(&entry.info_hash.to_hex(), vec![("x.mp4", 10, 10)]),
                Some(entry.added_by.to_hex()),
            );

            assert_eq!(media[0].added_by.as_deref(), Some(identity.device_id().to_hex().as_str()));
        }
    }
}
