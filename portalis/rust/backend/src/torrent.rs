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

/// One file to seed, as picked by Flutter (camera roll, file picker, etc.)
/// and passed across the FFI boundary as raw bytes — the only form that's
/// meaningfully the same file on every platform (mobile file pickers often
/// hand back a cache copy path, not a stable one worth trusting).
#[derive(Debug, Clone)]
pub struct NewFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Create a new collection by seeding local files: write them to disk,
/// build a `.torrent` from them, and add it back to the session pointed at
/// the same location — since the files are already there and match the
/// piece hashes just computed from them, librqbit verifies them as already
/// complete and starts seeding immediately, no download needed. This is
/// the "share something" side of the app; `add_torrent_from_*` above is
/// the "join a swarm" side — both produce the exact same `TorrentInfo`
/// shape either way (see the backend README on why: it's the same
/// protocol regardless of which side of the swarm you started on).
pub async fn create_collection(name: String, files: Vec<NewFile>) -> anyhow::Result<TorrentInfo> {
    native::create_collection(name, files).await
}

/// Add a torrent from a magnet link (or bare 40-char info-hash, which
/// `librqbit` also accepts as a magnet-equivalent).
pub async fn add_torrent_from_magnet(magnet_or_hash: String) -> anyhow::Result<TorrentInfo> {
    native::add_torrent_from_magnet(magnet_or_hash).await
}

/// Add a torrent from the raw bytes of a `.torrent` file (as picked by
/// Flutter's file picker and passed across the FFI boundary).
pub async fn add_torrent_from_file_bytes(bytes: Vec<u8>) -> anyhow::Result<TorrentInfo> {
    native::add_torrent_from_file_bytes(bytes).await
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

/// Caps upload speed across every torrent at once (not per-torrent) —
/// `librqbit`'s `Session::ratelimits` is adjustable at runtime, no restart
/// needed. `None`/0 means unlimited.
pub async fn set_upload_limit_bps(bytes_per_sec: Option<u32>) -> anyhow::Result<()> {
    native::set_upload_limit_bps(bytes_per_sec).await
}

/// The BitTorrent session's own listen port — for `collab_sync.rs` to
/// advertise in sync messages (so peers can fetch our seeded media
/// directly). Internal, never bridged (`pub(crate)` is invisible to FRB's
/// scan, same as `device::current_identity`).
pub(crate) async fn bt_listen_port() -> anyhow::Result<Option<u16>> {
    native::bt_listen_port().await
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

mod native {
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Context;
    use librqbit::api::TorrentIdOrHash;
    use librqbit::{
        Api, AddTorrent, AddTorrentOptions, AddTorrentResponse, CreateTorrentOptions, Session,
    };
    use tokio::sync::OnceCell;

    use super::{NewFile, TorrentFile, TorrentInfo};

    static SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

    async fn session() -> anyhow::Result<Arc<Session>> {
        SESSION
            .get_or_try_init(|| async {
                let dir = output_dir();
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("creating output dir {dir:?}"))?;
                let opts = librqbit::SessionOptions {
                    // Ask the router (UPnP/IGD) to forward our BT listen
                    // port, so peers outside this network can reach us —
                    // without it, cross-network fetches only work when the
                    // *other* side is reachable. Off in Session::new()'s
                    // defaults; harmless no-op on routers without UPnP.
                    enable_upnp_port_forwarding: true,
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
    fn sanitize_component(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .map(|c| match c {
                '/' | '\\' | '\0' => '_',
                c => c,
            })
            .collect();
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            "untitled".to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub(super) async fn create_collection(
        name: String,
        files: Vec<NewFile>,
    ) -> anyhow::Result<TorrentInfo> {
        anyhow::ensure!(!files.is_empty(), "a collection needs at least one file");

        let session = session().await?;
        let collection_name = sanitize_component(&name);
        let dir = output_dir().join(&collection_name);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating collection dir {dir:?}"))?;
        for file in &files {
            let path = dir.join(sanitize_component(&file.name));
            std::fs::write(&path, &file.bytes)
                .with_context(|| format!("writing {path:?} ({} bytes)", file.bytes.len()))?;
        }

        let created = librqbit::create_torrent(
            &dir,
            CreateTorrentOptions {
                name: Some(&collection_name),
                ..Default::default()
            },
        )
        .await
        .with_context(|| format!("building .torrent metadata from {dir:?}"))?;

        let opts = AddTorrentOptions {
            overwrite: true,
            // Explicit, not the session default + auto subfolder — the
            // files are already sitting exactly at `dir`, so this must
            // match precisely or librqbit will look for them in the wrong
            // place and try to re-download what we just wrote.
            output_folder: Some(dir.to_string_lossy().into_owned()),
            ..Default::default()
        };
        let response = session
            .add_torrent(AddTorrent::from_bytes(created.as_bytes()?), Some(opts))
            .await
            .with_context(|| format!("adding created torrent, output_folder={dir:?}"))?;
        response_to_info(&api(session), response)
    }

    /// Real per-file paths, resolved via `Api::api_torrent_details` rather
    /// than guessed — it already knows the exact output folder librqbit
    /// picked for this torrent (including the subfolder it auto-creates for
    /// multi-file torrents), which isn't reachable from `ManagedTorrent`'s
    /// own public API.
    fn files_for(api: &Api, id: usize, stats: &librqbit::TorrentStats) -> Vec<TorrentFile> {
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
                downloaded_bytes: stats.file_progress.get(i).copied().unwrap_or(0),
            })
            .collect()
    }

    fn to_info(api: &Api, id: usize, handle: &Arc<librqbit::ManagedTorrent>) -> TorrentInfo {
        let stats = handle.stats();
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
        let files = files_for(api, id, &stats);

        TorrentInfo {
            id,
            info_hash: handle.info_hash().as_string(),
            name: handle.name().unwrap_or_else(|| "(unnamed)".to_string()),
            state: format!("{:?}", stats.state),
            progress_bytes: stats.progress_bytes,
            total_bytes: stats.total_bytes,
            uploaded_bytes: stats.uploaded_bytes,
            download_mbps,
            upload_mbps,
            finished: stats.finished,
            error: stats.error.clone(),
            files,
            live_peers,
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

    pub(super) async fn add_torrent_from_file_bytes(
        bytes: Vec<u8>,
    ) -> anyhow::Result<TorrentInfo> {
        let session = session().await?;
        let response = session
            .add_torrent(AddTorrent::from_bytes(bytes), Some(add_opts()))
            .await
            .context("adding torrent from .torrent file bytes")?;
        response_to_info(&api(session), response)
    }

    pub(super) async fn list_torrents() -> anyhow::Result<Vec<TorrentInfo>> {
        let session = session().await?;
        let api = api(session.clone());
        Ok(session.with_torrents(|iter| {
            iter.map(|(id, handle)| to_info(&api, id, handle)).collect()
        }))
    }

    pub(super) async fn storage_usage_bytes() -> anyhow::Result<u64> {
        // Ensures output_dir() actually exists before walking it (a fresh
        // install with nothing downloaded yet shouldn't error, just read
        // as zero) — session() creates it as a side effect.
        let _ = session().await?;
        Ok(dir_size(&output_dir()))
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

    pub(super) async fn set_upload_limit_bps(bytes_per_sec: Option<u32>) -> anyhow::Result<()> {
        let session = session().await?;
        session
            .ratelimits
            .set_upload_bps(bytes_per_sec.and_then(std::num::NonZeroU32::new));
        Ok(())
    }

    pub(super) async fn bt_listen_port() -> anyhow::Result<Option<u16>> {
        let port = session().await?.tcp_listen_port();
        crate::log::clog!("torrent", "bt_listen_port: {port:?}");
        Ok(port)
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
}

