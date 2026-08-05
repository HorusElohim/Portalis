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
    /// Seconds until the download finishes at the current rate, or `None`
    /// when there is nothing meaningful to say: nothing left to fetch, or
    /// nothing moving to extrapolate from.
    ///
    /// Downloads only. Seeding has no endpoint to count down to — a peer's
    /// remaining bytes are their business and not visible from here — so an
    /// upload "ETA" would be a fabricated number.
    pub eta_secs: Option<u64>,
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
    /// The label of the *manifest entry* this file belongs to, as signed by
    /// whoever added it — the batch name, not this file's name. Several files
    /// share one. For a plain torrent it's the torrent's own name.
    ///
    /// Carried explicitly because flattening a collection's entries into a
    /// file list otherwise loses it entirely, leaving the UI to guess an
    /// entry's label from its first file.
    pub entry_name: String,
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

/// Whether the BitTorrent engine has finished starting.
///
/// Exposed so the UI can tell the difference between "this collection has
/// nothing to fetch" and "the engine hasn't come up yet, so nothing is being
/// shared *right now*". Those two look identical otherwise, which is exactly
/// the ambiguity that made a freshly-launched app look broken. Never blocks —
/// `sync_address`/`ensure_listener` warms the session in the background.
pub async fn engine_ready() -> bool {
    crate::substrate::current().ready()
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

/// Re-attempts fetches the user has asked for that haven't landed yet.
///
/// Driven by the reconciliation loop, since the thing that usually unblocks a
/// stalled fetch is a sync exchange telling us where the seeder's content
/// lives. Internal, never bridged.
pub(crate) async fn pursue_fetches() {
    native::pursue_fetches().await
}

/// Brings the engine up: the sync listener, the reconciliation loop, and the
/// BitTorrent session warming in the background.
///
/// Explicit, because it used to happen as a side effect of the UI asking for
/// the collection list — so the app's networking began when a screen first
/// drew, and an invite generated before that drew carried no address for
/// anyone to reach.
pub async fn start_engine() -> anyhow::Result<()> {
    crate::collab_sync::ensure_listener().await.map(|_| ())
}

/// Tells the engine whether anyone is looking, so it can stop reaching for
/// the network when nobody is. Called from the app's lifecycle observer.
pub async fn set_active(active: bool) {
    crate::reconciliation::set_reconciliation_active(active);
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

/// One item under the download directory, joined against whichever
/// collection actually claims it — the same join [`list_collections`] does
/// between the persisted manifest and the live session, applied to
/// `torrent::storage_breakdown`'s raw filesystem walk instead of to the
/// session's torrent list.
///
/// `collection_id`/`collection_name` are `None` when nothing in the app's
/// own state claims this path — the common case is a deleted collection's
/// leftovers: [`delete_collection`] deliberately leaves downloaded files on
/// disk, so a folder can easily outlive every record of what it was.
#[derive(Debug, Clone)]
pub struct StorageEntry {
    pub name: String,
    pub bytes: u64,
    pub path: String,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
}

/// What's on disk, resolved back to the collections the app knows about.
pub async fn storage_breakdown() -> anyhow::Result<Vec<StorageEntry>> {
    native::storage_breakdown().await
}

mod native {
    use std::collections::{HashMap, HashSet};

    use crate::collab_store::{read_store, with_store};
    use crate::domain::collaborator::{Collaborator, Role};
    use crate::domain::collection::{Collection, CollectionId};
    use crate::domain::invite::InviteSecret;
    use crate::domain::manifest::{InfoHash, ManifestEntry};
    use crate::log::clog;
    use crate::torrent::TorrentInfo;

    use anyhow::Context;

    use super::{CollaboratorInfo, CollectionInfo, CollectionKind, MediaInfo, StorageEntry};

    fn now_unix_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    /// Current sync endpoints to embed in invites: LAN always, public IP
    /// when discoverable. Starts the listener as a side effect, so a device
    /// that just generated an invite is already reachable.
    /// Where a peer can reach us, for embedding in an invite.
    ///
    /// Reads the listener rather than starting it. Listing collections used to
    /// bring up a socket, probe UPnP and warm the BitTorrent session as a side
    /// effect of the UI asking what exists — which is why nothing on this path
    /// could be tested without a network, and why an invite generated in the
    /// first second of a launch carried no addresses at all.
    fn current_sync_addresses() -> Vec<String> {
        let Some(addr) = crate::collab_sync::listening_at() else {
            clog!("collections", "current_sync_addresses: not listening yet");
            return Vec::new();
        };
        // Every real interface address, not just the default route's — a VPN
        // owning the default route would otherwise hide the Wi-Fi address the
        // peer can actually reach. See collab_sync::lan_ips.
        let mut addrs: Vec<String> = crate::collab_sync::lan_ips()
            .into_iter()
            .map(|ip| format!("{ip}:{}", addr.port()))
            .collect();
        if let Some(public) = crate::collab_sync::public_ip_now() {
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

    /// `entry_name` is the manifest entry's signed label; callers pass the
    /// torrent's own name for a plain torrent, which has no manifest entry.
    fn media_from_torrent(
        torrent: &TorrentInfo,
        added_by: Option<String>,
        entry_name: &str,
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
                    entry_name: entry_name.to_string(),
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

        /// Remaining bytes over the current rate.
        ///
        /// This is the same arithmetic librqbit does internally
        /// (`SpeedEstimator::add_snapshot` stores `remaining / bps`), against
        /// the same smoothed five-second figure it reports as `mbps` — its own
        /// `time_remaining` is unreachable from here because
        /// `DurationWithHumanReadable` keeps its `Duration` private. Doing it
        /// here is better anyway: a collection spans several torrents, and
        /// what someone waiting wants is one number for the whole thing, not
        /// one per torrent.
        fn eta_secs(&self) -> Option<u64> {
            let remaining = self.total_bytes.checked_sub(self.downloaded_bytes)?;
            if remaining == 0 {
                return None;
            }
            let bytes_per_second = self.download_mbps * 1024.0 * 1024.0;
            // No rate means no basis for an estimate. Saying nothing beats
            // showing a number that means "forever".
            if bytes_per_second <= 0.0 {
                return None;
            }
            let seconds = remaining as f64 / bytes_per_second;
            seconds.is_finite().then(|| seconds.ceil() as u64)
        }
    }

    fn state_for(
        totals: &Totals,
        pending_media: u32,
        media_len: usize,
        connecting: bool,
    ) -> String {
        if media_len == 0 {
            "empty".to_string()
        } else if connecting && totals.total_bytes == 0 {
            // Distinct from "pending", which means nobody has asked for this
            // media yet. "Connecting" means we are looking for the device that
            // holds it — the two look identical from bytes alone (0 of 0), and
            // conflating them is why a fetch in progress was indistinguishable
            // from a fetch that never started.
            "connecting".to_string()
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
        // Never *waits* for the engine. Constructing librqbit's session
        // bootstraps the DHT, probes UPnP and re-reads persisted torrents; the
        // old version awaited all of that on the UI poll, so the first listing
        // after launch could hang for seconds with the collection names
        // already sitting on disk. `ensure_listener` warms the session in the
        // background, and a later poll picks it up.
        let content = crate::substrate::current();
        let torrents = if content.ready() { content.holdings().await } else { Vec::new() };
        let addrs = current_sync_addresses();

        // Self-heal this device's own collaborator records. Collections
        // created before the nickname was wired up (or before it was last
        // changed) carry a stale copy of the name, and the collaborator list
        // is exactly what sync broadcasts to peers — so a stale entry keeps
        // announcing the wrong name until corrected. `rename_device` checks
        // read-only first, so in the steady state this costs one comparison
        // and never touches the file.
        if let (Ok(identity), Ok(me)) = (
            crate::device::current_identity(),
            crate::device::device_identity(),
        ) {
            match crate::collab_store::rename_device(&identity.device_id(), &me.nickname) {
                Ok(n) if n > 0 => clog!(
                    "collections",
                    "list_collections: corrected {n} stale collaborator record(s) to {:?}",
                    me.nickname
                ),
                Ok(_) => {}
                Err(e) => clog!("collections", "list_collections: couldn't reconcile this \
                     device's collaborator name ({e:#})"),
            }
        }

        let by_hash: HashMap<String, &TorrentInfo> = torrents
            .iter()
            .map(|t| (norm(&t.info_hash), t))
            .collect();

        let (mut result, claimed) = read_store(|collections| {
            let mut claimed: HashSet<String> = HashSet::new();
            let mut out = Vec::with_capacity(collections.len());
            for collection in collections.iter() {
                let mut totals = Totals::new();
                let mut media = Vec::new();
                let mut pending_media = 0u32;

                let mut connecting = false;

                for entry in collection.manifest().entries() {
                    let hash = norm(&entry.info_hash.to_hex());
                    claimed.insert(hash.clone());
                    match by_hash.get(&hash) {
                        Some(torrent) => {
                            totals.add(torrent);
                            media.extend(media_from_torrent(
                                torrent,
                                Some(entry.added_by.to_hex()),
                                &entry.name,
                            ));
                        }
                        None => {
                            // Known but not fetched: one placeholder standing
                            // for the whole entry. Its real file list isn't
                            // knowable until the torrent's metadata arrives.
                            pending_media += 1;
                            connecting |= is_fetching(&hash);
                            media.push(MediaInfo {
                                name: entry.name.clone(),
                                entry_name: entry.name.clone(),
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
                    state: state_for(&totals, pending_media, media.len(), connecting),
                    pending_media,
                    eta_secs: totals.eta_secs(),
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
            // A plain torrent has no manifest entry, so it stands as its own.
            let media = media_from_torrent(torrent, None, &torrent.name);
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
                state: state_for(&totals, 0, media.len(), false),
                pending_media: 0,
                eta_secs: totals.eta_secs(),
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
        // The device's *real* nickname, not a hardcoded "Me" — that literal
        // was what every collaborator list showed for this device, on this
        // device and on every peer it synced with, no matter what the user
        // had renamed themselves to.
        let nickname = crate::device::device_identity()?.nickname;
        let id = with_store(|collections| {
            let mut collection = Collection::new(name);
            collection.collaborators.push(Collaborator::new(
                identity.device_id(),
                nickname,
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
        match add_media_to_collection(created.id.clone(), name, files).await {
            Ok(info) => Ok(info),
            Err(e) => {
                // Roll back, or a failure here leaves a persisted empty
                // collection behind on top of the error the user already
                // sees — the same silent accumulation that produced piles of
                // indistinguishable duplicates before.
                clog!(
                    "collections",
                    "create_collection_with_media: adding media failed ({e:#}), \
                     removing the empty collection {}",
                    created.id
                );
                if let Err(cleanup) = delete_collection(created.id).await {
                    clog!("collections", "create_collection_with_media: rollback also \
                         failed ({cleanup:#}) — an empty collection may remain");
                }
                Err(e)
            }
        }
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
            // Rejoining an invite you already hold must land on the collection
            // you already have. Pushing a second record with the same
            // rendezvous key used to look harmless — two entries in a list —
            // but it silently broke sync forever: `apply_message` merges into
            // the *first* record with a matching key, so everything a peer
            // sent arrived in the older copy while the user watched the newer
            // one stay empty. And rejoining is exactly what someone does when
            // the first join appears not to have worked.
            if let Some(existing) = collections
                .iter_mut()
                .find(|c| c.rendezvous_key().to_hex() == rendezvous_key_hex)
            {
                clog!(
                    "collections",
                    "join_collection: already joined as {:?} ({}) — reusing it",
                    existing.name,
                    existing.id
                );
                // The invite is the authority on the name; ours may predate a
                // rename by the inviter.
                existing.name = name.to_string();
                if !existing
                    .collaborators
                    .iter()
                    .any(|c| c.device_id == identity.device_id())
                {
                    existing.collaborators.push(Collaborator::new(
                        identity.device_id(),
                        display_name.clone(),
                        Role::Member,
                        now_unix_ms(),
                    ));
                }
                return Ok(existing.id);
            }
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
        clog!("collections", "join_collection: local record is id={id}");

        // Best-effort first sync with the inviter, in the *background*. Each
        // candidate address can take up to the sync IO timeout to fail, and
        // an invite can carry two — awaiting that here made joining look like
        // the app had hung. The local record stands regardless of whether
        // those addresses turn out to be reachable; whoever is looking at the
        // collection picks up the result once/if the sync lands.
        if peer_addrs.is_empty() {
            clog!("collections", "join_collection: invite carried no addresses — nothing to \
                 sync with. The inviting device couldn't start its sync listener (on Android \
                 that means a build without the INTERNET permission); sync manually from the \
                 collection screen using the address on its User screen.");
        } else {
            // Where the peers are is the durable part; reaching them is the
            // loop's job. On iOS and macOS the very first connection to a LAN
            // address is the one the system spends raising a permission
            // prompt, and therefore the one that fails — which is exactly why
            // this must not be a single attempt fired by the join.
            crate::collab_sync::remember_sync_peers(&rendezvous_key_hex, peer_addrs);
            crate::reconciliation::request_reconciliation();
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
        let torrent_info = crate::substrate::current().publish(batch_dir_name, files).await?;
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
        crate::reconciliation::request_reconciliation();
        reload(&collection_id).await
    }

    /// Info-hashes whose `add_torrent` is still running.
    ///
    /// librqbit's `add_torrent` for a bare info-hash **awaits metadata
    /// resolution** — it connects to peers, pulls the `.torrent` info via
    /// `ut_metadata`, and only then returns a handle (verified in 8.1.1's
    /// `session.rs::add_torrent_internal`, which calls `resolve_magnet` with
    /// no timeout of its own). If no peer answers, that await never finishes.
    ///
    /// Fetching used to `await` it directly, so a fetch that couldn't find the
    /// seeder hung the FFI call forever: the Fetch button span, nothing
    /// appeared in the session, and the entry sat at 0% with nothing to say
    /// why. Tracking what is in flight lets the call return immediately
    /// without stacking a second attempt on top of the first.
    static FETCHING: std::sync::Mutex<Option<HashSet<String>>> = std::sync::Mutex::new(None);

    /// Long enough for a slow DHT lookup to land, short enough that a fetch
    /// which will never succeed can be retried in the same sitting.
    const METADATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

    /// `true` if this info-hash wasn't already being fetched.
    fn begin_fetch(info_hash: &str) -> bool {
        FETCHING
            .lock()
            .unwrap()
            .get_or_insert_with(HashSet::new)
            .insert(norm(info_hash))
    }

    fn end_fetch(info_hash: &str) {
        if let Some(set) = FETCHING.lock().unwrap().as_mut() {
            set.remove(&norm(info_hash));
        }
    }

    fn is_fetching(info_hash: &str) -> bool {
        FETCHING
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|set| set.contains(&norm(info_hash)))
    }

    /// Collections the user has asked to fetch and that still owe entries.
    ///
    /// Tapping "Fetch" used to be a single attempt: if no peer could be found
    /// at that instant — which is the normal case moments after joining, before
    /// the first sync has told us where the seeder's BitTorrent session is —
    /// nothing ever tried again, and the collection sat at 0% looking broken.
    /// A fetch is better understood as a standing intent than a one-off
    /// command, so it is remembered and retried on the sync tick until every
    /// entry has actually landed.
    static FETCH_REQUESTED: std::sync::Mutex<Option<HashSet<String>>> =
        std::sync::Mutex::new(None);

    fn note_fetch_requested(collection_id: &str) {
        FETCH_REQUESTED
            .lock()
            .unwrap()
            .get_or_insert_with(HashSet::new)
            .insert(collection_id.to_string());
    }

    fn forget_fetch_request(collection_id: &str) {
        if let Some(set) = FETCH_REQUESTED.lock().unwrap().as_mut() {
            set.remove(collection_id);
        }
    }

    /// Re-attempts every outstanding fetch. Called from the sync loop, right
    /// after an exchange that may just have taught us where a peer is.
    pub(super) async fn pursue_fetches() {
        let requested: Vec<String> = FETCH_REQUESTED
            .lock()
            .unwrap()
            .as_ref()
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        for collection_id in requested {
            match fetch_pending(&collection_id).await {
                Ok(0) => {}
                Ok(n) => clog!("collections", "retry: started {n} fetch(es) for {collection_id}"),
                // A collection deleted while a fetch was outstanding lands
                // here; dropping the request stops it retrying forever.
                Err(e) => {
                    clog!("collections", "retry: {collection_id} failed ({e:#}), giving up on it");
                    forget_fetch_request(&collection_id);
                }
            }
        }
    }

    pub(super) async fn fetch_collection_media(collection_id: String) -> anyhow::Result<u32> {
        clog!("collections", "fetch_collection_media: id={collection_id}");
        note_fetch_requested(&collection_id);
        // A pass reconciles before it pursues, so a collection with no known
        // peer yet gets one found for it rather than needing its own lookup
        // here. Returns what is outstanding, which is what "Fetch N" meant.
        crate::reconciliation::request_reconciliation();
        fetch_pending(&collection_id).await
    }

    /// What the manifest lists that this device does not hold.
    ///
    /// No peer lookup here any more: a reconciliation pass reconciles before it
    /// pursues, so by the time this runs the addresses are as good as they are
    /// going to get. The inline sync this used to do was a second, differently
    /// paced copy of the loop's first half.
    async fn fetch_pending(collection_id: &str) -> anyhow::Result<u32> {
        let (key, wanted) = manifest_of(collection_id)?;
        let missing = missing_from(&wanted).await;
        if missing.is_empty() {
            forget_fetch_request(collection_id);
            return Ok(0);
        }
        Ok(start_acquiring(&key, missing))
    }

    fn manifest_of(collection_id: &str) -> anyhow::Result<(String, Vec<String>)> {
        let id = CollectionId::from_string(collection_id)?;
        read_store(|collections| {
            let collection = collections
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow::anyhow!("no such collection"))?;
            Ok((
                collection.rendezvous_key().to_hex(),
                collection.manifest().entries().map(|e| e.info_hash.to_hex()).collect(),
            ))
        })
    }

    async fn missing_from(wanted: &[String]) -> Vec<String> {
        let held: HashSet<String> = crate::substrate::current()
            .holdings()
            .await
            .iter()
            .map(|t| norm(&t.info_hash))
            .collect();
        wanted.iter().filter(|h| !held.contains(&norm(h))).cloned().collect()
    }

    /// Spawned, never awaited — see FETCHING. Returns what was started.
    fn start_acquiring(rendezvous_key_hex: &str, missing: Vec<String>) -> u32 {
        let peers = crate::collab_sync::learned_bt_peers(rendezvous_key_hex);
        clog!("collections", "fetch: {} missing, {} peer hint(s)={peers:?}", missing.len(), peers.len());
        missing.into_iter().filter(|handle| begin_fetch(handle)).map(|handle| {
            tokio::spawn(acquire(handle, peers.clone()));
        }).count() as u32
    }

    async fn acquire(handle: String, peers: Vec<std::net::SocketAddr>) {
        let outcome = tokio::time::timeout(
            METADATA_TIMEOUT,
            crate::substrate::current().acquire(&handle, peers),
        )
        .await;
        end_fetch(&handle);
        report(&handle, outcome);
    }

    fn report(handle: &str, outcome: Result<anyhow::Result<TorrentInfo>, tokio::time::error::Elapsed>) {
        match outcome {
            Ok(Ok(info)) => clog!("collections", "fetch: {handle} resolved, {} file(s)", info.files.len()),
            Ok(Err(e)) => clog!("collections", "fetch: {handle} failed: {e:#}"),
            Err(_) => clog!("collections", "fetch: {handle} found nobody holding it within \
                 {METADATA_TIMEOUT:?} — is the other device running, on this network, and \
                 showing the collection? Tapping fetch again retries."),
        }
    }

    pub(super) async fn sync_collection(
        collection_id: String,
        peer_addr: String,
    ) -> anyhow::Result<CollectionInfo> {
        clog!("collections", "sync_collection: id={collection_id} peer_addr={peer_addr:?}");
        let id = CollectionId::from_string(&collection_id)?;
        let rendezvous_key_hex = read_store(|collections| {
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
        // Typed in once, remembered from then on — the periodic re-sync picks
        // these up, so a manual sync is a one-time introduction rather than
        // something to repeat every time the collection changes.
        crate::collab_sync::remember_sync_peers(&rendezvous_key_hex, peer_addrs.clone());
        crate::collab_sync::sync_with_any(&rendezvous_key_hex, &peer_addrs).await?;
        reload(&collection_id).await
    }

    /// Which of a deleted collection's torrents should leave the session:
    /// those no surviving collection still lists.
    ///
    /// The same info-hash can legitimately appear in two manifests — the
    /// manifest is a grow-only set keyed by info-hash and nothing stops two
    /// collections carrying the same entry — so forgetting a torrent purely
    /// because one collection let go of it would stop seeding media another
    /// collection is still showing.
    fn hashes_to_forget(removed: &[String], survivors: &HashSet<String>) -> Vec<String> {
        removed
            .iter()
            .filter(|h| !survivors.contains(&norm(h)))
            .cloned()
            .collect()
    }

    pub(super) async fn delete_collection(collection_id: String) -> anyhow::Result<()> {
        clog!("collections", "delete_collection: id={collection_id}");
        // A shared collection's id is a UUID; a plain torrent's is its
        // info-hash. Which one it parses as tells us where it lives.
        if let Ok(id) = CollectionId::from_string(&collection_id) {
            let (rendezvous_key_hex, orphaned) = with_store(|collections| {
                let Some(position) = collections.iter().position(|c| c.id == id) else {
                    anyhow::bail!("no such collection");
                };
                let removed = collections.remove(position);
                let removed_hashes: Vec<String> = removed
                    .manifest()
                    .entries()
                    .map(|e| e.info_hash.to_hex())
                    .collect();
                let survivors: HashSet<String> = collections
                    .iter()
                    .flat_map(|c| c.manifest().entries())
                    .map(|e| norm(&e.info_hash.to_hex()))
                    .collect();
                Ok((
                    removed.rendezvous_key().to_hex(),
                    hashes_to_forget(&removed_hashes, &survivors),
                ))
            })?;

            // Deleting the record alone left every one of the collection's
            // torrents in the session. Unclaimed, they resurfaced immediately
            // as plain-torrent collections named after their batch directory
            // (`<label>-<uuid>`) — so removing a collection appeared to
            // replace it with a differently-named copy of itself, and
            // librqbit's own persistence brought them back after a restart
            // too.
            clog!(
                "collections",
                "delete_collection: forgetting {} torrent(s) no other collection claims",
                orphaned.len()
            );
            for info_hash in orphaned {
                // Files stay on disk either way — `forget_torrent` is
                // librqbit's "forget", not "delete".
                if let Err(e) = crate::substrate::current().release(&info_hash).await {
                    // An entry that was never fetched has no torrent to
                    // forget, which is the common case, not an error.
                    clog!("collections", "delete_collection: {info_hash} wasn't in the                          session ({e:#})");
                }
            }
            forget_fetch_request(&collection_id);
            crate::collab_sync::forget_collection_peers(&rendezvous_key_hex);
            return Ok(());
        }
        crate::substrate::current().release(&collection_id).await
    }

    pub(super) async fn sync_address() -> anyhow::Result<String> {
        let addrs = current_sync_addresses();
        anyhow::ensure!(!addrs.is_empty(), "couldn't start the sync listener");
        Ok(addrs.join(","))
    }

    pub(super) async fn storage_breakdown() -> anyhow::Result<Vec<StorageEntry>> {
        let raw = crate::torrent::storage_breakdown().await?;
        // The walk above already starts the session (see
        // torrent::storage_breakdown), so holdings() here is never what
        // brings it up — safe to ask for unconditionally, unlike
        // list_collections's own guard against exactly that.
        let torrents = crate::substrate::current().holdings().await;

        // Which top-level directory each live torrent's files sit under, so
        // a raw filesystem entry can be traced back to the torrent — and
        // from there, the collection — it belongs to. `starts_with` rather
        // than an exact match: a multi-file torrent nests its files under
        // subdirectories of the batch folder, not directly inside it.
        let by_path: Vec<(std::path::PathBuf, &TorrentInfo)> = torrents
            .iter()
            .filter_map(|t| {
                t.files.first().map(|f| (std::path::PathBuf::from(&f.absolute_path), t))
            })
            .collect();

        read_store(|collections| {
            Ok(raw
                .into_iter()
                .map(|entry| {
                    let entry_path = std::path::Path::new(&entry.path);
                    let owner = by_path
                        .iter()
                        .find(|(file_path, _)| file_path.starts_with(entry_path))
                        .map(|(_, t)| resolve_owner(t, collections))
                        .unwrap_or((None, None));
                    StorageEntry {
                        name: entry.name,
                        bytes: entry.bytes,
                        path: entry.path,
                        collection_id: owner.0,
                        collection_name: owner.1,
                    }
                })
                .collect())
        })
    }

    /// Whichever collection claims `torrent`: the shared collection whose
    /// manifest lists its info-hash, or — nothing claiming it — the
    /// plain-torrent collection it is its own one of. Mirrors exactly how
    /// `list_collections` tells the two apart.
    fn resolve_owner(
        torrent: &TorrentInfo,
        collections: &[Collection],
    ) -> (Option<String>, Option<String>) {
        let hash = norm(&torrent.info_hash);
        if let Some(c) = collections
            .iter()
            .find(|c| c.manifest().entries().any(|e| norm(&e.info_hash.to_hex()) == hash))
        {
            return (Some(c.id.to_string()), Some(c.name.clone()));
        }
        (Some(hash), Some(torrent.name.clone()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::identity::DeviceIdentity;

        /// Joining, twice, against a real store — reachable at all only now
        /// that listing has no network side effect.
        ///
        /// The second join is the bug: it used to push another record with the
        /// same rendezvous key, and `apply_message` merges into the first
        /// match — so everything a peer sent landed in the copy the user was
        /// not looking at. Rejoining is exactly what someone does when the
        /// first join appears not to have worked.
        #[tokio::test]
        async fn rejoining_lands_on_the_collection_you_already_have() {
            let _temp = crate::paths::redirect_to_temp();
            crate::collab_store::forget_cache_for_test();
            let invite = invite_code_for(&Collection::new("Trip".into()), &[]);

            let first = join_collection(invite.clone(), "Me".into()).await.unwrap();
            let again = join_collection(invite, "Me".into()).await.unwrap();

            assert_eq!(first.id, again.id);
            assert_eq!(list_collections().await.unwrap().len(), 1);
            // And we are in it once, not twice.
            assert_eq!(again.collaborators.len(), 1);
        }

        /// A collection with nothing fetched still describes itself: the
        /// entries are known because a peer signed them, and saying so is the
        /// difference between "nothing here" and "nothing here yet".
        #[tokio::test]
        async fn a_manifest_with_no_local_content_lists_as_pending() {
            let _temp = crate::paths::redirect_to_temp();
            crate::collab_store::forget_cache_for_test();
            let identity = crate::device::current_identity().unwrap();
            let created = create_collection("Trip".into()).await.unwrap();
            let id = CollectionId::from_string(&created.id).unwrap();
            with_store(|collections| {
                let collection = collections.iter_mut().find(|c| c.id == id).unwrap();
                collection.add_manifest_entry(ManifestEntry::new_signed(
                    InfoHash::from_bytes([8; 20]),
                    "Beach day".into(),
                    None,
                    &identity,
                    1,
                ));
                Ok(())
            })
            .unwrap();

            let listed = list_collections().await.unwrap();

            assert_eq!(listed[0].pending_media, 1);
            assert_eq!(listed[0].state, "pending");
            assert_eq!(listed[0].media[0].entry_name, "Beach day");
            assert!(!listed[0].media[0].fetched);
            // Nobody is listening, so an invite honestly carries no address.
            assert!(listed[0].invite_code.is_some());
        }

        /// Two entries, one already held: only the other is asked for, and
        /// asking twice does not stack a second attempt on the first.
        #[tokio::test]
        async fn fetching_asks_only_for_what_is_missing_and_only_once() {
            let _temp = crate::paths::redirect_to_temp();
            crate::collab_store::forget_cache_for_test();
            let content = std::sync::Arc::new(crate::substrate::Recorded::default());
            *content.held.lock().unwrap() = vec!["aa".repeat(20)];
            let _double = crate::substrate::use_double(content.clone());
            let id = seeded_with(&["aa".repeat(20), "bb".repeat(20)]).await;

            assert_eq!(fetch_collection_media(id.clone()).await.unwrap(), 1);
            // In flight, so the second tap adds nothing.
            assert_eq!(fetch_collection_media(id).await.unwrap(), 0);

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            assert_eq!(*content.acquired.lock().unwrap(), vec!["bb".repeat(20)]);
        }

        /// Deleting releases the content nothing else claims, and leaves what
        /// another collection still lists — the manifest is a grow-only set
        /// keyed by handle, so two collections may hold the same entry.
        #[tokio::test]
        async fn deleting_releases_only_what_no_other_collection_lists() {
            let _temp = crate::paths::redirect_to_temp();
            crate::collab_store::forget_cache_for_test();
            let content = std::sync::Arc::new(crate::substrate::Recorded::default());
            let _double = crate::substrate::use_double(content.clone());
            let shared = "cc".repeat(20);
            let doomed = seeded_with(&[shared.clone(), "dd".repeat(20)]).await;
            seeded_with(std::slice::from_ref(&shared)).await;

            delete_collection(doomed).await.unwrap();

            assert_eq!(*content.released.lock().unwrap(), vec!["dd".repeat(20)]);
        }

        /// A collection carrying these entries, signed by this device.
        async fn seeded_with(handles: &[String]) -> String {
            let identity = crate::device::current_identity().unwrap();
            let created = create_collection("Trip".into()).await.unwrap();
            let id = CollectionId::from_string(&created.id).unwrap();
            with_store(|collections| {
                let collection = collections.iter_mut().find(|c| c.id == id).unwrap();
                for handle in handles {
                    let bytes: [u8; 20] = hex::decode(handle).unwrap().try_into().unwrap();
                    collection.add_manifest_entry(ManifestEntry::new_signed(
                        InfoHash::from_bytes(bytes),
                        "batch".into(),
                        None,
                        &identity,
                        1,
                    ));
                }
                Ok(())
            })
            .unwrap();
            created.id
        }

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

            let media = media_from_torrent(&t, Some("device".into()), "batch label");

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
            assert_eq!(state_for(&complete, 1, 2, false), "downloading");
            assert_eq!(state_for(&complete, 0, 1, false), "seeding");
            assert_eq!(state_for(&Totals::new(), 3, 3, false), "pending");
            assert_eq!(state_for(&Totals::new(), 0, 0, false), "empty");
            // A fetch is running but no metadata has arrived, so there are
            // still no bytes to report — indistinguishable from "pending"
            // without this.
            assert_eq!(state_for(&Totals::new(), 3, 3, true), "connecting");
            // Once anything is actually moving, that is the more useful thing
            // to say.
            assert_eq!(state_for(&complete, 1, 2, true), "downloading");
        }

        #[test]
        fn eta_is_remaining_bytes_over_the_current_rate() {
            let mut totals = Totals::new();
            // 100 MiB total, half done, moving at 1 MiB/s => 50s left.
            totals.add(&torrent("aa", vec![("a", 100 * 1024 * 1024, 50 * 1024 * 1024)]));
            totals.download_mbps = 1.0;

            assert_eq!(totals.eta_secs(), Some(50));
        }

        #[test]
        fn eta_is_absent_when_there_is_nothing_honest_to_say() {
            // Stalled: a rate of zero extrapolates to "never", and a number
            // meaning never is worse than no number.
            let mut stalled = Totals::new();
            stalled.add(&torrent("aa", vec![("a", 100, 10)]));
            assert_eq!(stalled.eta_secs(), None);

            // Complete: nothing left to wait for, whatever the rate.
            let mut done = Totals::new();
            done.add(&torrent("bb", vec![("b", 100, 100)]));
            done.download_mbps = 5.0;
            assert_eq!(done.eta_secs(), None);
        }

        #[test]
        fn deleting_a_collection_forgets_only_torrents_nothing_else_claims() {
            // The bug this guards: deleting a collection left its torrents in
            // the session, where they immediately came back as plain-torrent
            // collections named after their batch directory.
            let removed = vec!["AABB".to_string(), "ccdd".to_string()];
            let survivors: HashSet<String> = ["aabb".to_string()].into_iter().collect();

            let forget = hashes_to_forget(&removed, &survivors);

            // Still listed by another collection — it must keep seeding, and
            // the case difference must not hide that.
            assert_eq!(forget, vec!["ccdd".to_string()]);
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
                &entry.name,
            );

            assert_eq!(media[0].added_by.as_deref(), Some(identity.device_id().to_hex().as_str()));
            // The entry's signed label survives the flattening into files —
            // the file is "x.mp4" but it belongs to the "RAW_3000" batch.
            assert_eq!(media[0].name, "x.mp4");
            assert_eq!(media[0].entry_name, "RAW_3000");
        }
    }
}
