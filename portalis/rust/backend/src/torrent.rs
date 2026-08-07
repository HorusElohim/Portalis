//! Debug-only torrent smoke test: add a magnet link or raw `.torrent` file
//! and watch it download via a bare `librqbit::Session`, bypassing the
//! Collection/Manifest domain model entirely. This exists to validate the
//! `librqbit` integration itself (the thing `rust/backend/README.md` calls
//! `SwarmEngine`) works end-to-end before building the real ports/adapters
//! around it. Not part of the production API surface.
//!
//! The DTO and function signatures below are unconditional (needed on every
//! target, wasm32 included) because `flutter_rust_bridge`'s generated glue
//! (`src/api.rs`) references `crate::torrent::*` regardless of this
//! module's own visibility or any `#[cfg]` on its declaration — only the
//! *implementation* is target-gated, falling back to an error on wasm32
//! (Web is a viewer, not a swarm participant; see the backend README).

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub id: usize,
    pub info_hash: String,
    pub name: String,
    pub state: String,
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub finished: bool,
    pub error: Option<String>,
    pub files: Vec<TorrentFile>,
    /// Currently-connected peers (not the historical total ever seen) — the
    /// closest real equivalent to the mockup's "N copies alive" indicator.
    pub live_peers: u32,
    /// `"ip:port"` of each currently-connected peer. BitTorrent peers carry
    /// no identity beyond their network address — there is no name, no
    /// signed device id, nothing to correlate them with a collaborator.
    pub live_peer_addrs: Vec<String>,
}

/// One file inside a torrent, with its real on-disk location — resolved via
/// `librqbit::Api` (which knows the actual per-torrent output folder,
/// including the subfolder it auto-creates for multi-file torrents) rather
/// than guessed, so Flutter can load a thumbnail or open/play it directly.
#[derive(Debug, Clone)]
pub struct TorrentFile {
    pub name: String,
    pub absolute_path: String,
    pub length_bytes: u64,
    pub downloaded_bytes: u64,
}

/// One native file Rust will publish as torrent content.
///
/// Only the path crosses the Flutter boundary. Rust owns reading, hashing,
/// torrent construction, and seeding, so file size is never limited by Dart
/// memory or the bridge's signed 32-bit byte-vector encoding. Publication
/// links this source into its torrent layout; it never copies its bytes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourceFile {
    pub name: String,
    pub path: String,
    pub length_bytes: Option<u64>,
}

pub(crate) fn make_source_names_unique(files: &mut [SourceFile]) {
    let mut names = std::collections::HashSet::new();
    for file in files {
        let original = native::sanitize_component(&file.name);
        let mut candidate = original.clone();
        let mut suffix = 2;
        while !names.insert(candidate.to_lowercase()) {
            candidate = numbered_source_name(&original, suffix);
            suffix += 1;
        }
        file.name = candidate;
    }
}

fn numbered_source_name(name: &str, suffix: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            format!("{stem} ({suffix}).{extension}")
        }
        _ => format!("{name} ({suffix})"),
    }
}

pub(crate) fn validate_source_files(files: &[SourceFile]) -> anyhow::Result<u64> {
    anyhow::ensure!(!files.is_empty(), "a collection needs at least one file");
    let mut names = std::collections::HashSet::new();
    let mut total_bytes = 0u64;
    for file in files {
        let name = native::sanitize_component(&file.name);
        anyhow::ensure!(
            names.insert(name.to_lowercase()),
            "duplicate source filename {:?}",
            file.name
        );
        let location = crate::content_location::ContentLocation::from_source_path(&file.path)?;
        let length = location.length(file.length_bytes)?;
        total_bytes = total_bytes
            .checked_add(length)
            .ok_or_else(|| anyhow::anyhow!("source size overflow"))?;
    }
    Ok(total_bytes)
}

pub(crate) async fn inspect_source_files(files: &[SourceFile]) -> anyhow::Result<u64> {
    let files = files.to_vec();
    tokio::task::spawn_blocking(move || validate_source_files(&files))
        .await
        .map_err(|error| anyhow::anyhow!("source inspection task failed: {error}"))?
}

pub(crate) const TORRENT_PIECE_LENGTH: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct PublishProgress {
    inner: std::sync::Arc<std::sync::Mutex<PublishProgressState>>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone)]
pub(crate) struct PublishProgressState {
    pub(crate) stage: String,
    pub(crate) processed_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) completed_pieces: u64,
    pub(crate) total_pieces: u64,
    pub(crate) error: Option<String>,
}

impl PublishProgress {
    pub(crate) fn new(total_bytes: u64) -> Self {
        Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(PublishProgressState {
                stage: "preparing".into(),
                processed_bytes: 0,
                total_bytes,
                completed_pieces: 0,
                total_pieces: total_bytes.div_ceil(TORRENT_PIECE_LENGTH),
                error: None,
            })),
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(crate) fn snapshot(&self) -> PublishProgressState {
        self.inner.lock().unwrap().clone()
    }

    pub(crate) fn set_stage(&self, stage: &str) {
        let mut state = self.inner.lock().unwrap();
        if stage == "hashing" && state.stage != "hashing" {
            state.processed_bytes = 0;
            state.completed_pieces = 0;
        }
        state.stage = stage.into();
    }

    fn advance(&self, bytes: u64) {
        let mut state = self.inner.lock().unwrap();
        state.processed_bytes = state
            .processed_bytes
            .saturating_add(bytes)
            .min(state.total_bytes);
    }

    fn advance_hashing(&self, bytes: u64, completed_piece: bool) {
        let mut state = self.inner.lock().unwrap();
        state.processed_bytes = state
            .processed_bytes
            .saturating_add(bytes)
            .min(state.total_bytes);
        if completed_piece {
            state.completed_pieces = state
                .completed_pieces
                .saturating_add(1)
                .min(state.total_pieces);
        }
    }

    fn complete_final_piece(&self) {
        let mut state = self.inner.lock().unwrap();
        state.completed_pieces = state
            .completed_pieces
            .saturating_add(1)
            .min(state.total_pieces);
    }

    pub(crate) fn fail(&self, error: String) {
        let mut state = self.inner.lock().unwrap();
        state.stage = "failed".into();
        state.error = Some(error);
    }

    pub(crate) fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn ensure_active(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.cancelled.load(std::sync::atomic::Ordering::Relaxed),
            "publication cancelled"
        );
        Ok(())
    }
}

#[cfg(test)]
mod validation_tests {
    use super::{
        make_source_names_unique, native::sanitize_component, validate_source_files, SourceFile,
    };
    use std::io::Write;

    #[test]
    fn accepts_a_path_source_without_a_size_cap() {
        let path = std::env::temp_dir().join(format!("portalis-source-{}", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&[0; 1024]).unwrap();
        assert_eq!(
            validate_source_files(&[SourceFile {
                name: "photo.jpg".into(),
                path: path.to_string_lossy().into_owned(),
                length_bytes: None,
            }])
            .unwrap(),
            1024
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_an_empty_share() {
        assert!(validate_source_files(&[]).is_err());
    }

    #[test]
    fn duplicate_source_names_get_stable_suffixes() {
        let mut files = vec![
            SourceFile {
                name: "photo.jpg".into(),
                path: "one".into(),
                length_bytes: None,
            },
            SourceFile {
                name: "photo.jpg".into(),
                path: "two".into(),
                length_bytes: None,
            },
            SourceFile {
                name: "Photo.jpg".into(),
                path: "three".into(),
                length_bytes: None,
            },
        ];

        make_source_names_unique(&mut files);

        assert_eq!(files[0].name, "photo.jpg");
        assert_eq!(files[1].name, "photo (2).jpg");
        assert_eq!(files[2].name, "Photo (3).jpg");
    }

    #[test]
    fn source_names_are_safe_and_portable() {
        assert_eq!(sanitize_component("../bad:name?.jpg"), "_bad_name_.jpg");
        assert_eq!(sanitize_component("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_component("..."), "untitled");
    }
}

/// Create a new collection by linking local files into a torrent layout,
/// building a `.torrent` from that layout and adding it back to the session at
/// the same location — since the files are already there and match the
/// piece hashes just computed from them, librqbit verifies them as already
/// complete and starts seeding immediately, no download needed. This is
/// the "share something" side of the app; `add_torrent_from_*` above is
/// the "join a swarm" side — both produce the exact same `TorrentInfo`
/// shape either way (see the backend README on why: it's the same
/// protocol regardless of which side of the swarm you started on).
pub async fn create_collection(
    name: String,
    files: Vec<SourceFile>,
) -> anyhow::Result<TorrentInfo> {
    let total = inspect_source_files(&files).await?;
    native::create_collection(name, files, PublishProgress::new(total)).await
}

pub(crate) async fn publish(
    name: String,
    files: Vec<SourceFile>,
    progress: PublishProgress,
) -> anyhow::Result<TorrentInfo> {
    native::create_collection(name, files, progress).await
}

/// Add a torrent from a magnet link (or bare 40-char info-hash, which
/// `librqbit` also accepts as a magnet-equivalent).
pub async fn add_torrent_from_magnet(magnet_or_hash: String) -> anyhow::Result<TorrentInfo> {
    native::add_torrent_from_magnet(magnet_or_hash).await
}

/// Add a `.torrent` file without loading its metadata into Dart first.
pub async fn add_torrent_from_file_path(path: String) -> anyhow::Result<TorrentInfo> {
    native::add_torrent_from_file_path(path).await
}

/// Snapshot of every torrent currently managed by the debug session. The
/// Flutter side polls this on a timer — this is a smoke test, not the
/// push-based `watch_*` design the real Collections API will use.
pub async fn list_torrents() -> anyhow::Result<Vec<TorrentInfo>> {
    native::list_torrents().await
}

/// Where downloaded files actually land, so the UI can show the user a real
/// path instead of leaving them to guess (this was a temp directory before —
/// invisible in practice). A real desktop `MediaStorageSink` (see the
/// backend README) will replace this later; for this smoke test it's just
/// the platform Downloads folder.
pub fn output_dir() -> anyhow::Result<String> {
    Ok(native::output_dir().display().to_string())
}

/// Real disk usage of everything downloaded/shared so far — the Settings
/// screen's storage meter. Recursive over `output_dir()`.
pub async fn storage_usage_bytes() -> anyhow::Result<u64> {
    native::storage_usage_bytes().await
}

/// One top-level item under the download directory — in practice, almost
/// always one manifest entry's own batch folder (see
/// `collections::add_media_to_collection`) or a plain torrent's folder.
/// `bytes` is recursive, so a multi-file batch reports what it actually
/// costs on disk.
///
/// Internal: `collections::storage_breakdown` is the bridged version, which
/// joins each of these against the collection that actually claims it — the
/// same join `list_collections` does between the persisted manifest and the
/// live session, so it belongs there and not here. See that module's doc for
/// why the join can only happen in the one layer that sees both sides.
#[derive(Debug, Clone)]
pub(crate) struct RawStorageEntry {
    pub(crate) name: String,
    pub(crate) bytes: u64,
    pub(crate) path: String,
}

/// What's actually on disk under the download directory, one entry per
/// top-level item, largest first — the real filesystem, where
/// `storage_usage_bytes`'s single total can only say "this much, somewhere".
pub(crate) async fn storage_breakdown() -> anyhow::Result<Vec<RawStorageEntry>> {
    native::storage_breakdown().await
}

/// Caps transfer speed across every torrent at once (not per-torrent).
/// `librqbit`'s `Session::ratelimits` is adjustable at runtime, so unlike the
/// rest of `SessionOptions` these take effect without a restart. `None` means
/// unlimited. Internal: `settings.rs` owns the user-facing surface, so there
/// is one way to change a rate limit rather than two.
pub(crate) async fn set_rate_limits(
    upload_bps: Option<u32>,
    download_bps: Option<u32>,
) -> anyhow::Result<()> {
    native::set_rate_limits(upload_bps, download_bps).await
}

/// Whether the BitTorrent session has already been constructed. Lets
/// `settings.rs` tell the difference between "this change needs a restart"
/// and "the session hasn't started yet, so it will pick this up anyway".
pub(crate) fn session_started() -> bool {
    native::session_started()
}

/// The BitTorrent session's own listen port — for `collab_sync.rs` to
/// advertise in sync messages (so peers can fetch our seeded media
/// directly). Internal, never bridged (`pub(crate)` is invisible to FRB's
/// scan, same as `device::current_identity`).
pub(crate) async fn bt_listen_port() -> anyhow::Result<Option<u16>> {
    native::bt_listen_port().await
}

/// The BitTorrent listen port if it has *ever* been read in this run, without
/// touching the session.
///
/// The port is fixed for the life of the process, but the only way to ask for
/// it — `session()` — blocks while librqbit starts up (DHT bootstrap, UPnP
/// probe, re-reading persisted torrents). `collab_sync` therefore only waits a
/// couple of seconds for it and sends `None` on timeout, and a sync message
/// without a port leaves the other side with no direct address to fetch media
/// from: it falls back to DHT, which on a LAN behind one NAT typically never
/// resolves. Caching turns that into a once-per-run race instead of one that
/// can be lost on every single exchange.
pub(crate) fn bt_listen_port_cached() -> Option<u16> {
    native::bt_listen_port_cached()
}

/// Adds a torrent by bare info-hash with explicit peer-address hints —
/// `collab_sync.rs`'s learned "who has this collection's media" addresses
/// go straight to librqbit as `initial_peers`, so a LAN fetch connects to
/// the seeder immediately instead of waiting on DHT discovery.
pub(crate) async fn add_info_hash_with_peers(
    info_hash_hex: &str,
    peers: Vec<std::net::SocketAddr>,
) -> anyhow::Result<TorrentInfo> {
    native::add_info_hash_with_peers(info_hash_hex, peers).await
}

/// Removes a torrent from the session, leaving its downloaded files on disk
/// (librqbit's "forget", as opposed to "delete" which also unlinks them).
/// Backs deleting a plain-torrent collection — see
/// `collections::delete_collection`.
pub(crate) async fn forget_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
    native::forget_torrent(info_hash_hex).await?;
    crate::linked_source_store::remove(info_hash_hex)?;
    Ok(())
}

pub(crate) async fn pause_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
    native::pause_torrent(info_hash_hex).await
}

pub(crate) async fn restart_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
    native::restart_torrent(info_hash_hex).await
}

pub(crate) async fn delete_torrent_files(info_hash_hex: &str) -> anyhow::Result<()> {
    native::delete_torrent_files(info_hash_hex).await
}

/// Rebuilds no-copy source storage after a process restart. Session persistence
/// cannot serialize a custom storage factory, so Portalis restores it itself.
pub(crate) async fn restore_linked_sources() -> anyhow::Result<()> {
    native::restore_linked_sources().await
}

/// Pauses a linked torrent as soon as its original source disappears.
pub(crate) async fn verify_linked_sources() {
    native::verify_linked_sources().await;
}

mod native {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Context;
    use bencode::bencode_serialize_to_writer;
    use buffers::ByteBufOwned;
    use librqbit::api::TorrentIdOrHash;
    use librqbit::storage::{StorageFactory, StorageFactoryExt, TorrentStorage};
    use librqbit::{
        AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, CreateTorrentOptions,
        ManagedTorrentShared, Session, TorrentMetadata,
    };
    use librqbit_core::torrent_metainfo::{
        TorrentMetaV1File, TorrentMetaV1Info, TorrentMetaV1Owned,
    };
    use librqbit_core::Id20;
    use sha1w::{ISha1, Sha1};
    use tokio::sync::OnceCell;

    use super::{
        PublishProgress, RawStorageEntry, SourceFile, TorrentFile, TorrentInfo,
        TORRENT_PIECE_LENGTH,
    };

    static SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

    /// The precise place librqbit hashes and seeds. A single selected file is
    /// already a valid torrent root; several independent files need a shared
    /// directory, represented by hard links rather than copied content.
    struct SeedLayout {
        hash_path: PathBuf,
        output_folder: PathBuf,
        link_directory: Option<PathBuf>,
        linked_paths: Vec<PathBuf>,
        torrent_name: Option<String>,
    }

    #[derive(Clone)]
    struct ReferencedStorageFactory {
        sources: Vec<crate::content_location::ContentLocation>,
        lengths: Vec<u64>,
    }

    impl StorageFactory for ReferencedStorageFactory {
        type Storage = ReferencedStorage;

        fn create(
            &self,
            _shared: &ManagedTorrentShared,
            _metadata: &TorrentMetadata,
        ) -> anyhow::Result<Self::Storage> {
            Ok(ReferencedStorage {
                sources: self.sources.clone(),
                lengths: self.lengths.clone(),
            })
        }

        fn clone_box(&self) -> librqbit::storage::BoxStorageFactory {
            self.clone().boxed()
        }
    }

    #[derive(Clone)]
    struct ReferencedStorage {
        sources: Vec<crate::content_location::ContentLocation>,
        lengths: Vec<u64>,
    }

    impl TorrentStorage for ReferencedStorage {
        fn init(
            &mut self,
            _shared: &ManagedTorrentShared,
            _metadata: &TorrentMetadata,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        fn pread_exact(
            &self,
            file_id: usize,
            offset: u64,
            buffer: &mut [u8],
        ) -> anyhow::Result<()> {
            self.sources
                .get(file_id)
                .context("no such referenced source file")?
                .read_exact_at(offset, buffer)
        }

        fn pwrite_all(&self, _file_id: usize, _offset: u64, _buffer: &[u8]) -> anyhow::Result<()> {
            anyhow::bail!("gallery-linked source storage is read-only")
        }

        fn remove_file(&self, _file_id: usize, _filename: &std::path::Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn remove_directory_if_empty(&self, _path: &std::path::Path) -> anyhow::Result<()> {
            Ok(())
        }

        fn ensure_file_length(&self, file_id: usize, length: u64) -> anyhow::Result<()> {
            anyhow::ensure!(
                self.lengths.get(file_id) == Some(&length),
                "referenced source length changed"
            );
            Ok(())
        }

        fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
            Ok(Box::new(self.clone()))
        }
    }

    async fn session() -> anyhow::Result<Arc<Session>> {
        SESSION
            .get_or_try_init(|| async {
                // Read once so the configured output folder and all other
                // session construction settings are from the same snapshot.
                let settings = crate::settings::engine_settings().unwrap_or_default();
                let dir = output_dir_for(&settings);
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("creating output dir {dir:?}"))?;
                // Every knob comes from the persisted settings now — see
                // settings.rs. librqbit reads these once, here, which is why
                // changing any of them needs a restart.
                let peer_opts = librqbit::PeerConnectionOptions {
                    connect_timeout: settings
                        .peer_connect_timeout_secs
                        .map(|s| std::time::Duration::from_secs(s as u64)),
                    read_write_timeout: settings
                        .peer_read_write_timeout_secs
                        .map(|s| std::time::Duration::from_secs(s as u64)),
                    keep_alive_interval: settings
                        .peer_keep_alive_interval_secs
                        .map(|s| std::time::Duration::from_secs(s as u64)),
                };
                let trackers = settings
                    .trackers
                    .iter()
                    .filter_map(|t| match t.parse() {
                        Ok(url) => Some(url),
                        Err(e) => {
                            crate::log::clog!("torrent", "ignoring unparseable tracker {t:?}: {e}");
                            None
                        }
                    })
                    .collect();
                let opts = librqbit::SessionOptions {
                    // A range is mandatory, not optional: librqbit only binds
                    // a TCP listener `if let Some(port_range) =
                    // opts.listen_port_range` (verified in 8.1.1's
                    // session.rs) — with none it binds nothing, this device
                    // can never accept an incoming peer connection, and
                    // enable_upnp_port_forwarding has no port to forward.
                    // settings.rs rejects an empty or zero range for exactly
                    // that reason.
                    listen_port_range: Some(settings.listen_port_start..settings.listen_port_end),
                    enable_upnp_port_forwarding: settings.enable_upnp_port_forwarding,
                    // Without persistence librqbit starts empty every launch,
                    // so a collection you were seeding comes back with every
                    // manifest entry unmatched and silently seeds nothing.
                    persistence: settings.persist_session.then_some(
                        librqbit::SessionPersistenceConfig::Json {
                            // OS-specific default folder (inside the app's
                            // container on macOS/iOS), like our other state.
                            folder: None,
                        },
                    ),
                    fastresume: settings.fastresume,
                    disable_dht: settings.disable_dht,
                    disable_dht_persistence: settings.disable_dht_persistence,
                    socks_proxy_url: settings.socks_proxy_url.clone(),
                    defer_writes_up_to: settings.defer_writes_up_to_mb.map(|mb| mb as usize),
                    concurrent_init_limit: settings.concurrent_init_limit.map(|n| n as usize),
                    peer_opts: Some(peer_opts),
                    blocklist_url: settings.blocklist_url.clone(),
                    trackers,
                    ratelimits: librqbit::limits::LimitsConfig {
                        upload_bps: settings
                            .upload_limit_bps
                            .and_then(std::num::NonZeroU32::new),
                        download_bps: settings
                            .download_limit_bps
                            .and_then(std::num::NonZeroU32::new),
                    },
                    ..Default::default()
                };
                Session::new_with_opts(dir.clone(), opts)
                    .await
                    .with_context(|| format!("starting librqbit session in {dir:?}"))
            })
            .await
            .cloned()
    }

    /// Where downloaded files land, chosen so the user can actually find
    /// them:
    /// - Desktop: the platform Downloads folder (falls back to a temp dir
    ///   if that can't be found, e.g. some minimal Linux setups).
    /// - iOS/Android: there's no "Downloads folder" in an app's sandbox —
    ///   that's a desktop concept. `dirs::download_dir()` on iOS resolves
    ///   to `<sandbox>/Downloads`, a directory nothing ever exposes to the
    ///   user; the actual user-visible location is `Documents` (surfaced in
    ///   the Files app once `UIFileSharingEnabled` +
    ///   `LSSupportsOpeningDocumentsInPlace` are set in Info.plist, which
    ///   they now are).
    pub(super) fn output_dir() -> PathBuf {
        let settings = crate::settings::engine_settings().unwrap_or_default();
        output_dir_for(&settings)
    }

    fn source_link_dir(name: &str) -> PathBuf {
        crate::paths::state_dir().join("source-links").join(name)
    }

    fn output_dir_for(settings: &crate::settings::EngineSettings) -> PathBuf {
        if let Some(dir) = settings
            .download_dir
            .as_deref()
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
        {
            return PathBuf::from(dir);
        }

        #[cfg(any(target_os = "ios", target_os = "android"))]
        let base = dirs::document_dir().unwrap_or_else(std::env::temp_dir);
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let base = dirs::download_dir().unwrap_or_else(std::env::temp_dir);

        base.join("Portalis-TorrentDebug")
    }

    /// Re-adding the same torrent (e.g. re-testing) shouldn't blow up just
    /// because its files already exist on disk from a previous run.
    fn add_opts() -> AddTorrentOptions {
        AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        }
    }

    /// Strip path separators and other characters that would let a
    /// malicious/odd filename escape the collection's own directory.
    pub(super) fn sanitize_component(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .map(|c| match c {
                '/' | '\\' | '\0' | '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();
        let trimmed = cleaned.trim().trim_matches(['.', ' ']);
        if trimmed.is_empty() {
            return "untitled".to_string();
        }
        let stem = trimmed.split('.').next().unwrap_or_default().to_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        );
        if reserved {
            format!("_{trimmed}")
        } else {
            trimmed.to_string()
        }
    }

    pub(super) async fn create_collection(
        name: String,
        files: Vec<SourceFile>,
        progress: PublishProgress,
    ) -> anyhow::Result<TorrentInfo> {
        let session = session().await?;
        let collection_name = sanitize_component(&name);
        let sources = files
            .iter()
            .map(|file| crate::content_location::ContentLocation::from_source_path(&file.path))
            .collect::<anyhow::Result<Vec<_>>>()?;
        if sources
            .iter()
            .any(crate::content_location::ContentLocation::requires_native_storage)
        {
            return create_referenced_collection(
                session,
                collection_name,
                files,
                sources,
                progress,
            )
            .await;
        }
        let layout = if let [file] = files.as_slice() {
            let hash_path = crate::content_location::ContentLocation::from_source_path(&file.path)?
                .filesystem_path()
                .to_path_buf();
            let output_folder = hash_path
                .parent()
                .context("a source file must have a parent directory")?
                .to_path_buf();
            SeedLayout {
                hash_path,
                output_folder,
                link_directory: None,
                linked_paths: Vec::new(),
                // A single-file torrent's root name must remain its filename
                // so librqbit finds the selected file in its original folder.
                torrent_name: None,
            }
        } else {
            // Multi-file torrents need one common filesystem root for
            // librqbit, but this internal layout is made of hard links, not
            // downloaded content. Keep the directory entries in Portalis'
            // private state rather than making them look like copied files in
            // the user's Downloads folder.
            let dir = source_link_dir(&collection_name);
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("creating collection dir {dir:?}"))?;
            progress.set_stage("linking");
            let layout_dir = dir.clone();
            let layout_files = files.clone();
            let layout_progress = progress.clone();
            let linked_paths = match tokio::task::spawn_blocking(move || {
                link_sources(&layout_dir, &layout_files, &layout_progress)
            })
            .await
            .context("source linking task failed")
            {
                Ok(result) => result,
                Err(error) => return Err(error),
            }?;
            SeedLayout {
                hash_path: dir.clone(),
                output_folder: dir.clone(),
                link_directory: Some(dir),
                linked_paths,
                torrent_name: Some(collection_name),
            }
        };

        if let Err(error) = progress.ensure_active() {
            discard_seed_layout(&layout);
            return Err(error);
        }
        progress.set_stage("hashing");
        let created = match librqbit::create_torrent(
            &layout.hash_path,
            CreateTorrentOptions {
                name: layout.torrent_name.as_deref(),
                ..Default::default()
            },
        )
        .await
        .with_context(|| format!("building .torrent metadata from {:?}", layout.hash_path))
        {
            Ok(created) => created,
            Err(error) => {
                discard_seed_layout(&layout);
                return Err(error);
            }
        };

        if let Err(error) = progress.ensure_active() {
            discard_seed_layout(&layout);
            return Err(error);
        }
        progress.set_stage("seeding");
        let opts = AddTorrentOptions {
            overwrite: true,
            // Explicit, not the session default + auto subfolder — the
            // files are already sitting at the canonical output folder, so this must
            // match precisely or librqbit will look for them in the wrong
            // place and try to re-download what we just wrote.
            output_folder: Some(layout.output_folder.to_string_lossy().into_owned()),
            ..Default::default()
        };
        let torrent_bytes = match created
            .as_bytes()
            .context("encoding created torrent metadata")
        {
            Ok(bytes) => bytes,
            Err(error) => {
                discard_seed_layout(&layout);
                return Err(error);
            }
        };
        let response = match session
            .add_torrent(AddTorrent::from_bytes(torrent_bytes), Some(opts))
            .await
            .with_context(|| {
                format!(
                    "adding created torrent, output_folder={:?}",
                    layout.output_folder
                )
            }) {
            Ok(response) => response,
            Err(error) => {
                discard_seed_layout(&layout);
                return Err(error);
            }
        };
        response_to_info(&api(session), response)
    }

    async fn create_referenced_collection(
        session: Arc<Session>,
        collection_name: String,
        files: Vec<SourceFile>,
        sources: Vec<crate::content_location::ContentLocation>,
        progress: PublishProgress,
    ) -> anyhow::Result<TorrentInfo> {
        let lengths = files
            .iter()
            .zip(&sources)
            .map(|(file, source)| source.length(file.length_bytes))
            .collect::<anyhow::Result<Vec<_>>>()?;
        progress.set_stage("hashing");
        let hash_lengths = lengths.clone();
        let torrent_bytes = tokio::task::spawn_blocking({
            let files = files.clone();
            let sources = sources.clone();
            let progress = progress.clone();
            let collection_name = collection_name.clone();
            move || {
                create_referenced_metainfo(
                    &collection_name,
                    &files,
                    &sources,
                    &hash_lengths,
                    &progress,
                )
            }
        })
        .await
        .context("referenced source hashing task failed")??;

        progress.ensure_active()?;
        progress.set_stage("seeding");
        let metadata_dir = source_link_dir(&collection_name);
        std::fs::create_dir_all(&metadata_dir)?;
        let options = AddTorrentOptions {
            overwrite: true,
            output_folder: Some(metadata_dir.to_string_lossy().into_owned()),
            storage_factory: Some(ReferencedStorageFactory { sources, lengths }.boxed()),
            ..Default::default()
        };
        let persisted_bytes = torrent_bytes.to_vec();
        let response = session
            .add_torrent(AddTorrent::from_bytes(torrent_bytes), Some(options))
            .await
            .context("adding gallery-linked torrent")?;
        let info = response_to_info(&api(session), response)?;
        crate::linked_source_store::upsert(crate::linked_source_store::LinkedSourceRecord {
            info_hash: info.info_hash.clone(),
            torrent_bytes: persisted_bytes,
            sources: files,
        })?;
        Ok(info)
    }

    pub(super) async fn restore_linked_sources() -> anyhow::Result<()> {
        let session = session().await?;
        for record in crate::linked_source_store::load()? {
            if crate::torrent::inspect_source_files(&record.sources)
                .await
                .is_err()
            {
                crate::log::clog!(
                    "torrent",
                    "linked source unavailable after restart: {}",
                    record.info_hash
                );
                continue;
            }
            let id = TorrentIdOrHash::try_from(record.info_hash.as_str())?;
            if session.get(id).is_some() {
                session.delete(id, false).await?;
            }
            let sources = record
                .sources
                .iter()
                .map(|source| {
                    crate::content_location::ContentLocation::from_source_path(&source.path)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let lengths = record
                .sources
                .iter()
                .zip(&sources)
                .map(|(source, location)| location.length(source.length_bytes))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let metadata_dir = source_link_dir(&record.info_hash);
            std::fs::create_dir_all(&metadata_dir)?;
            session
                .add_torrent(
                    AddTorrent::from_bytes(record.torrent_bytes),
                    Some(AddTorrentOptions {
                        overwrite: true,
                        output_folder: Some(metadata_dir.to_string_lossy().into_owned()),
                        storage_factory: Some(
                            ReferencedStorageFactory { sources, lengths }.boxed(),
                        ),
                        ..Default::default()
                    }),
                )
                .await
                .with_context(|| format!("restoring linked source {}", record.info_hash))?;
        }
        Ok(())
    }

    pub(super) async fn verify_linked_sources() {
        let Ok(records) = crate::linked_source_store::load() else {
            return;
        };
        for record in records {
            if crate::torrent::inspect_source_files(&record.sources)
                .await
                .is_ok()
            {
                continue;
            }
            crate::log::clog!("torrent", "linked source unavailable: {}", record.info_hash);
            let _ = pause_torrent(&record.info_hash).await;
        }
    }

    fn create_referenced_metainfo(
        collection_name: &str,
        files: &[SourceFile],
        sources: &[crate::content_location::ContentLocation],
        lengths: &[u64],
        progress: &PublishProgress,
    ) -> anyhow::Result<bytes::Bytes> {
        const PIECE_LENGTH: u32 = TORRENT_PIECE_LENGTH as u32;
        const READ_SIZE: usize = 64 * 1024;
        anyhow::ensure!(
            files.len() == sources.len() && files.len() == lengths.len(),
            "source descriptor mismatch"
        );
        let mut remaining = PIECE_LENGTH as usize;
        let mut checksum = Sha1::new();
        let mut pieces = Vec::new();
        let mut output_files = Vec::with_capacity(files.len());
        let mut buffer = vec![0; READ_SIZE];
        for ((file, source), length) in files.iter().zip(sources).zip(lengths) {
            let mut offset = 0;
            while offset < *length {
                progress.ensure_active()?;
                let size = ((*length - offset) as usize)
                    .min(remaining)
                    .min(buffer.len());
                source.read_exact_at(offset, &mut buffer[..size])?;
                checksum.update(&buffer[..size]);
                offset += size as u64;
                remaining -= size;
                progress.advance_hashing(size as u64, remaining == 0);
                if remaining == 0 {
                    pieces.extend_from_slice(&checksum.finish());
                    checksum = Sha1::new();
                    remaining = PIECE_LENGTH as usize;
                }
            }
            output_files.push(TorrentMetaV1File {
                length: *length,
                path: vec![sanitize_component(&file.name).into_bytes().into()],
                attr: None,
                sha1: None,
                symlink_path: None,
            });
        }
        if remaining < PIECE_LENGTH as usize {
            pieces.extend_from_slice(&checksum.finish());
            progress.complete_final_piece();
        }
        let single = files.len() == 1;
        let info = TorrentMetaV1Info::<ByteBufOwned> {
            name: Some(
                (if single {
                    sanitize_component(&files[0].name)
                } else {
                    collection_name.into()
                })
                .into_bytes()
                .into(),
            ),
            pieces: pieces.into(),
            piece_length: PIECE_LENGTH,
            length: single.then_some(lengths[0]),
            md5sum: None,
            files: (!single).then_some(output_files),
            attr: None,
            sha1: None,
            symlink_path: None,
            private: false,
        };
        let info_hash = hash_metainfo(&info)?;
        let metainfo = TorrentMetaV1Owned {
            announce: None,
            announce_list: Vec::new(),
            info,
            comment: None,
            created_by: None,
            encoding: Some(b"utf-8".as_slice().into()),
            publisher: None,
            publisher_url: None,
            creation_date: None,
            info_hash,
        };
        let mut bytes = Vec::new();
        bencode_serialize_to_writer(&metainfo, &mut bytes)?;
        Ok(bytes.into())
    }

    fn hash_metainfo(info: &TorrentMetaV1Info<ByteBufOwned>) -> anyhow::Result<Id20> {
        struct DigestWriter(Sha1);
        impl Write for DigestWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = DigestWriter(Sha1::new());
        bencode_serialize_to_writer(info, &mut writer)?;
        Ok(Id20::new(writer.0.finish()))
    }

    /// Only paths created by this publication may be removed on failure. The
    /// old recursive cleanup could erase a pre-existing collection directory.
    fn discard_seed_layout(layout: &SeedLayout) {
        if let Some(dir) = &layout.link_directory {
            discard_linked_sources(dir, &layout.linked_paths);
        }
    }

    fn discard_linked_sources(dir: &std::path::Path, created: &[PathBuf]) {
        for path in created.iter().rev() {
            if let Err(error) = std::fs::remove_file(path) {
                crate::log::clog!(
                    "torrent",
                    "could not remove linked source {path:?}: {error}"
                );
            }
        }
        let _ = std::fs::remove_dir(dir);
    }

    /// Builds the collection directory using hard links only. This creates a
    /// second name for the same filesystem bytes, never a second file. A
    /// hard-link failure is deliberately surfaced instead of falling back to
    /// a copy: callers can choose a source on the same filesystem or a
    /// canonical Portalis destination, but Portalis must not silently violate
    /// its one-copy contract.
    fn link_sources(
        dir: &std::path::Path,
        files: &[SourceFile],
        progress: &PublishProgress,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let mut created = Vec::with_capacity(files.len());
        for file in files {
            progress.ensure_active()?;
            let source = crate::content_location::ContentLocation::from_source_path(&file.path)?
                .filesystem_path()
                .to_path_buf();
            let destination = dir.join(sanitize_component(&file.name));
            let length = std::fs::metadata(&source)
                .with_context(|| format!("reading source metadata {source:?}"))?
                .len();

            if source != destination {
                if let Err(error) = std::fs::hard_link(&source, &destination) {
                    if !is_resumable_link(&destination, length) {
                        discard_linked_sources(dir, &created);
                        return Err(error).with_context(|| {
                            format!(
                                "linking {source:?} into {destination:?}; Portalis will not copy source bytes"
                            )
                        });
                    }
                } else {
                    created.push(destination);
                }
            }
            progress.advance(length);
        }
        Ok(created)
    }

    /// Import batches use a UUID in their layout directory name. If a process
    /// was interrupted after making a link, the matching regular file and
    /// length identify that durable batch layout without rereading gigabytes.
    /// Torrent hashing remains the single content-verification pass.
    fn is_resumable_link(path: &std::path::Path, length: u64) -> bool {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.len() == length)
            .unwrap_or(false)
    }

    /// Real per-file paths, resolved via `Api::api_torrent_details` rather
    /// than guessed — it already knows the exact output folder librqbit
    /// picked for this torrent (including the subfolder it auto-creates for
    /// multi-file torrents), which isn't reachable from `ManagedTorrent`'s
    /// own public API.
    fn files_for(
        api: &Api,
        id: usize,
        stats: &librqbit::TorrentStats,
        initializing: bool,
    ) -> Vec<TorrentFile> {
        let Ok(details) = api.api_torrent_details(TorrentIdOrHash::Id(id)) else {
            return Vec::new();
        };
        let Some(files) = details.files else {
            return Vec::new();
        };
        let output_folder = std::path::Path::new(&details.output_folder);
        files
            .into_iter()
            .enumerate()
            .map(|(i, f)| TorrentFile {
                absolute_path: output_folder.join(&f.name).to_string_lossy().into_owned(),
                name: f.name,
                length_bytes: f.length,
                // See `to_info`: file_progress isn't meaningful yet either
                // while the startup integrity scan is still running.
                downloaded_bytes: if initializing {
                    0
                } else {
                    stats.file_progress.get(i).copied().unwrap_or(0)
                },
            })
            .collect()
    }

    fn to_info(api: &Api, id: usize, handle: &Arc<librqbit::ManagedTorrent>) -> TorrentInfo {
        let stats = handle.stats();
        let state = format!("{:?}", stats.state);
        // While a resumed torrent is being verified at startup (state
        // "Initializing", with or without fastresume), librqbit's
        // `progress_bytes` is the scan's own read cursor — bytes *scanned*
        // so far, matched or not — not bytes confirmed to actually match
        // their hash (see librqbit's `TorrentStateLocked::stats`, the
        // `Initializing` arm, backed by `checked_bytes` in
        // `file_ops::initial_check`, which is incremented unconditionally
        // per piece before the hash comparison). That climbs to the full
        // size by the time the scan finishes regardless of how much of a
        // partially-downloaded file actually matched, so a collection could
        // flash "complete" for as long as the check takes. The real,
        // have-bitmap-derived figure only exists once the state moves on to
        // Paused/Live, so until then this reports nothing rather than a
        // number that lies in the optimistic direction.
        let initializing = state == "Initializing";
        let (download_mbps, upload_mbps, live_peers) = stats
            .live
            .as_ref()
            .map(|live| {
                (
                    live.download_speed.mbps,
                    live.upload_speed.mbps,
                    live.snapshot.peer_stats.live as u32,
                )
            })
            .unwrap_or((0.0, 0.0, 0));
        let files = files_for(api, id, &stats, initializing);
        // `PeerStatsFilter`/`PeerStatsSnapshot` are private-in-public on
        // `Api::api_peer_stats` (librqbit 8.1.1) — reachable through type
        // inference and field access without ever naming them, same as
        // librqbit's own HTTP API layer consumes it.
        //
        // Gated on `stats.live` rather than called unconditionally: this is
        // polled every 500ms per torrent (see `CollectionsController`'s
        // active-poll interval), and `api_peer_stats` fails with exactly
        // "not live" whenever `handle.live()` is `None` — the same thing
        // `stats.live` already tells us. Calling it anyway for every
        // checking/queued/peerless torrent turned "not live yet" (routine,
        // and true for most of a torrent's life) into a log line every poll
        // tick for as long as it stayed that way. Skipping the call when we
        // already know it's doomed keeps the log for what it's for: a
        // torrent `stats` calls live still failing peer stats, which would
        // be the actual anomaly.
        let live_peer_addrs: Vec<String> = if stats.live.is_some() {
            match api.api_peer_stats(TorrentIdOrHash::Id(id), Default::default()) {
                Ok(snapshot) => snapshot.peers.into_keys().collect(),
                Err(error) => {
                    crate::log::clog!(
                        "torrent",
                        "peer_stats unavailable for {}: {error:#}",
                        handle.info_hash().as_string()
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        TorrentInfo {
            id,
            info_hash: handle.info_hash().as_string(),
            name: handle.name().unwrap_or_else(|| "(unnamed)".to_string()),
            state,
            progress_bytes: if initializing {
                0
            } else {
                stats.progress_bytes
            },
            total_bytes: stats.total_bytes,
            uploaded_bytes: stats.uploaded_bytes,
            download_mbps,
            upload_mbps,
            finished: stats.finished,
            error: stats.error.clone(),
            files,
            live_peers,
            live_peer_addrs,
        }
    }

    fn response_to_info(api: &Api, response: AddTorrentResponse) -> anyhow::Result<TorrentInfo> {
        let (id, handle) = match response {
            AddTorrentResponse::Added(id, handle) => (id, handle),
            AddTorrentResponse::AlreadyManaged(id, handle) => (id, handle),
            AddTorrentResponse::ListOnly(_) => {
                anyhow::bail!("torrent was added in list-only mode; no handle available")
            }
        };
        Ok(to_info(api, id, &handle))
    }

    fn api(session: Arc<Session>) -> Api {
        Api::new(session, None)
    }

    pub(super) async fn add_torrent_from_magnet(
        magnet_or_hash: String,
    ) -> anyhow::Result<TorrentInfo> {
        let session = session().await?;
        let response = session
            .add_torrent(AddTorrent::from_url(magnet_or_hash), Some(add_opts()))
            .await
            .context("adding torrent from magnet/URL")?;
        response_to_info(&api(session), response)
    }

    pub(super) async fn add_torrent_from_file_bytes(bytes: Vec<u8>) -> anyhow::Result<TorrentInfo> {
        let session = session().await?;
        let response = session
            .add_torrent(AddTorrent::from_bytes(bytes), Some(add_opts()))
            .await
            .context("adding torrent from .torrent file bytes")?;
        response_to_info(&api(session), response)
    }

    pub(super) async fn add_torrent_from_file_path(path: String) -> anyhow::Result<TorrentInfo> {
        let read_path = path.clone();
        let bytes = tokio::task::spawn_blocking(move || std::fs::read(&read_path))
            .await
            .context("torrent metadata read task failed")?
            .with_context(|| format!("reading .torrent metadata from {path:?}"))?;
        add_torrent_from_file_bytes(bytes).await
    }

    pub(super) async fn list_torrents() -> anyhow::Result<Vec<TorrentInfo>> {
        let session = session().await?;
        let api = api(session.clone());
        Ok(session
            .with_torrents(|iter| iter.map(|(id, handle)| to_info(&api, id, handle)).collect()))
    }

    /// How long a walk stays good for. Both the Settings screen's total (this
    /// function, polled every 2s while it's open — see
    /// `SettingsScreen._storagePoll`) and the Storage screen's breakdown
    /// (`collections::storage_breakdown`, same cadence — see
    /// `StorageScreen._poll`) read through this one cache, so having both
    /// open at once still costs one walk, not two. Same short-TTL idiom as
    /// `collab_sync::lan_ips`.
    const STORAGE_TTL: std::time::Duration = std::time::Duration::from_secs(5);
    static STORAGE_CACHE: std::sync::Mutex<Option<(std::time::Instant, Vec<RawStorageEntry>)>> =
        std::sync::Mutex::new(None);

    pub(super) async fn storage_usage_bytes() -> anyhow::Result<u64> {
        Ok(storage_breakdown().await?.iter().map(|e| e.bytes).sum())
    }

    pub(super) async fn storage_breakdown() -> anyhow::Result<Vec<RawStorageEntry>> {
        // Ensures output_dir() actually exists before walking it (a fresh
        // install with nothing downloaded yet shouldn't error, just read
        // as empty) — session() creates it as a side effect.
        let _ = session().await?;

        if let Some((at, cached)) = STORAGE_CACHE.lock().unwrap().as_ref() {
            if at.elapsed() < STORAGE_TTL {
                return Ok(cached.clone());
            }
        }
        let dir = output_dir();
        // A recursive stat walk is blocking I/O; running it inline would tie
        // up a tokio worker thread for however long the disk takes.
        let entries = tokio::task::spawn_blocking(move || {
            let Ok(read) = std::fs::read_dir(&dir) else {
                return Vec::new();
            };
            let mut entries: Vec<RawStorageEntry> = read
                .filter_map(|e| e.ok())
                .map(|entry| {
                    let path = entry.path();
                    let bytes = if path.is_dir() {
                        dir_size(&path)
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };
                    RawStorageEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        bytes,
                        path: path.to_string_lossy().into_owned(),
                    }
                })
                .collect();
            entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
            entries
        })
        .await?;
        *STORAGE_CACHE.lock().unwrap() = Some((std::time::Instant::now(), entries.clone()));
        Ok(entries)
    }

    fn dir_size(path: &std::path::Path) -> u64 {
        let Ok(entries) = std::fs::read_dir(path) else {
            return 0;
        };
        entries
            .filter_map(|e| e.ok())
            .map(|entry| {
                let Ok(metadata) = entry.metadata() else {
                    return 0;
                };
                if metadata.is_dir() {
                    dir_size(&entry.path())
                } else {
                    metadata.len()
                }
            })
            .sum()
    }

    pub(super) fn session_started() -> bool {
        SESSION.initialized()
    }

    pub(super) async fn set_rate_limits(
        upload_bps: Option<u32>,
        download_bps: Option<u32>,
    ) -> anyhow::Result<()> {
        // Deliberately doesn't force the session to start: if it hasn't yet,
        // the limits are already in the SessionOptions it will be built from
        // (see `session()`), and starting a whole BitTorrent session as a
        // side effect of saving a preference would be a surprising cost.
        if !session_started() {
            crate::log::clog!(
                "torrent",
                "set_rate_limits: session not started yet — the saved values \
                 will be applied when it is"
            );
            return Ok(());
        }
        let session = session().await?;
        session
            .ratelimits
            .set_upload_bps(upload_bps.and_then(std::num::NonZeroU32::new));
        session
            .ratelimits
            .set_download_bps(download_bps.and_then(std::num::NonZeroU32::new));
        crate::log::clog!(
            "torrent",
            "set_rate_limits: upload={upload_bps:?} download={download_bps:?} bytes/sec"
        );
        Ok(())
    }

    /// 0 = not yet known. The port never changes once the session is up, so a
    /// plain atomic is enough — see `torrent::bt_listen_port_cached`.
    static BT_LISTEN_PORT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    pub(super) async fn bt_listen_port() -> anyhow::Result<Option<u16>> {
        let port = session().await?.tcp_listen_port();
        if let Some(port) = port {
            BT_LISTEN_PORT.store(port as u32, std::sync::atomic::Ordering::Relaxed);
        }
        crate::log::clog!("torrent", "bt_listen_port: {port:?}");
        Ok(port)
    }

    pub(super) fn bt_listen_port_cached() -> Option<u16> {
        match BT_LISTEN_PORT.load(std::sync::atomic::Ordering::Relaxed) {
            0 => None,
            port => Some(port as u16),
        }
    }

    pub(super) async fn forget_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
        crate::log::clog!("torrent", "forget_torrent: info_hash={info_hash_hex}");
        let session = session().await?;
        let id = TorrentIdOrHash::try_from(info_hash_hex)
            .map_err(|e| anyhow::anyhow!("{info_hash_hex} isn't a valid info hash: {e}"))?;
        if session.get(id).is_none() {
            return Ok(());
        }
        // `false` = forget only. Downloaded files stay on disk: removing a
        // collection from the app shouldn't silently destroy the user's
        // media, which `delete(.., true)` would.
        session
            .delete(id, false)
            .await
            .context("forgetting torrent")?;
        Ok(())
    }

    pub(super) async fn pause_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
        let session = session().await?;
        let id = TorrentIdOrHash::try_from(info_hash_hex)
            .map_err(|e| anyhow::anyhow!("{info_hash_hex} isn't a valid info hash: {e}"))?;
        let Some(handle) = session.get(id) else {
            return Ok(());
        };
        session.pause(&handle).await.context("pausing torrent")
    }

    pub(super) async fn restart_torrent(info_hash_hex: &str) -> anyhow::Result<()> {
        let session = session().await?;
        let id = TorrentIdOrHash::try_from(info_hash_hex)
            .map_err(|e| anyhow::anyhow!("{info_hash_hex} isn't a valid info hash: {e}"))?;
        if let Some(handle) = session.get(id) {
            return session.unpause(&handle).await.context("restarting torrent");
        }

        session
            .add_torrent(AddTorrent::from_url(info_hash_hex), Some(add_opts()))
            .await
            .with_context(|| format!("re-adding torrent {info_hash_hex}"))?;
        Ok(())
    }

    pub(super) async fn delete_torrent_files(info_hash_hex: &str) -> anyhow::Result<()> {
        crate::log::clog!("torrent", "delete_torrent_files: info_hash={info_hash_hex}");
        let session = session().await?;
        let id = TorrentIdOrHash::try_from(info_hash_hex)
            .map_err(|e| anyhow::anyhow!("{info_hash_hex} isn't a valid info hash: {e}"))?;
        if session.get(id).is_none() {
            return Ok(());
        }
        session
            .delete(id, true)
            .await
            .context("deleting torrent files")
    }

    pub(super) async fn add_info_hash_with_peers(
        info_hash_hex: &str,
        peers: Vec<std::net::SocketAddr>,
    ) -> anyhow::Result<TorrentInfo> {
        crate::log::clog!(
            "torrent",
            "add_info_hash_with_peers: info_hash={info_hash_hex} peer_hints={peers:?}"
        );
        let session = session().await?;
        let opts = AddTorrentOptions {
            overwrite: true,
            initial_peers: (!peers.is_empty()).then_some(peers),
            ..Default::default()
        };
        let response = session
            // A bare 40-char hex info-hash is a valid magnet identifier to
            // librqbit's URL parser, same as the user-facing magnet flow.
            .add_torrent(AddTorrent::from_url(info_hash_hex), Some(opts))
            .await
            .with_context(|| format!("adding torrent {info_hash_hex} with peer hints"))?;
        response_to_info(&api(session), response)
    }

    #[cfg(test)]
    mod link_tests {
        use super::{link_sources, PublishProgress, SourceFile};

        #[test]
        fn source_layout_is_a_link_not_a_second_file() {
            let root =
                std::env::temp_dir().join(format!("portalis-links-{}", uuid::Uuid::new_v4()));
            let source_dir = root.join("source");
            let layout_dir = root.join("layout");
            std::fs::create_dir_all(&source_dir).unwrap();
            std::fs::create_dir_all(&layout_dir).unwrap();
            let source = source_dir.join("clip.mp4");
            std::fs::write(&source, b"first").unwrap();

            let files = [SourceFile {
                name: "clip.mp4".into(),
                path: source.to_string_lossy().into_owned(),
                length_bytes: None,
            }];
            let created = link_sources(&layout_dir, &files, &PublishProgress::new(5)).unwrap();
            assert_eq!(created, vec![layout_dir.join("clip.mp4")]);

            // A write through the original name is visible through the
            // collection name: this proves the layout does not contain a
            // copied byte sequence.
            std::fs::write(&source, b"again").unwrap();
            assert_eq!(
                std::fs::read(layout_dir.join("clip.mp4")).unwrap(),
                b"again"
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
