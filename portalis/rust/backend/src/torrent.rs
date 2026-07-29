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

/// Add a torrent from a magnet link (or bare 40-char info-hash, which
/// `librqbit` also accepts as a magnet-equivalent).
pub async fn add_torrent_from_magnet(magnet_or_hash: String) -> anyhow::Result<TorrentInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::add_torrent_from_magnet(magnet_or_hash).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = magnet_or_hash;
        native::unsupported_on_web()
    }
}

/// Add a torrent from the raw bytes of a `.torrent` file (as picked by
/// Flutter's file picker and passed across the FFI boundary).
pub async fn add_torrent_from_file_bytes(bytes: Vec<u8>) -> anyhow::Result<TorrentInfo> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::add_torrent_from_file_bytes(bytes).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = bytes;
        native::unsupported_on_web()
    }
}

/// Snapshot of every torrent currently managed by the debug session. The
/// Flutter side polls this on a timer — this is a smoke test, not the
/// push-based `watch_*` design the real Collections API will use.
pub async fn list_torrents() -> anyhow::Result<Vec<TorrentInfo>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::list_torrents().await
    }
    #[cfg(target_arch = "wasm32")]
    {
        native::unsupported_on_web()
    }
}

/// Where downloaded files actually land, so the UI can show the user a real
/// path instead of leaving them to guess (this was a temp directory before —
/// invisible in practice). A real desktop `MediaStorageSink` (see the
/// backend README) will replace this later; for this smoke test it's just
/// the platform Downloads folder.
pub fn output_dir() -> anyhow::Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(native::output_dir().display().to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        native::unsupported_on_web()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::PathBuf;
    use std::sync::Arc;

    use librqbit::api::TorrentIdOrHash;
    use librqbit::{Api, AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};
    use tokio::sync::OnceCell;

    use super::{TorrentFile, TorrentInfo};

    static SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

    async fn session() -> anyhow::Result<Arc<Session>> {
        SESSION
            .get_or_try_init(|| async {
                let dir = output_dir();
                std::fs::create_dir_all(&dir)?;
                Session::new(dir).await
            })
            .await
            .cloned()
    }

    /// The platform Downloads folder (falling back to a temp dir if it
    /// can't be found, e.g. some minimal Linux setups), so downloaded files
    /// are somewhere the user can actually find in Finder/Explorer rather
    /// than an OS temp directory.
    pub(super) fn output_dir() -> PathBuf {
        dirs::download_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("SmartShare-TorrentDebug")
    }

    /// Re-adding the same torrent (e.g. re-testing) shouldn't blow up just
    /// because its files already exist on disk from a previous run.
    fn add_opts() -> AddTorrentOptions {
        AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        }
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
            .await?;
        response_to_info(&api(session), response)
    }

    pub(super) async fn add_torrent_from_file_bytes(
        bytes: Vec<u8>,
    ) -> anyhow::Result<TorrentInfo> {
        let session = session().await?;
        let response = session
            .add_torrent(AddTorrent::from_bytes(bytes), Some(add_opts()))
            .await?;
        response_to_info(&api(session), response)
    }

    pub(super) async fn list_torrents() -> anyhow::Result<Vec<TorrentInfo>> {
        let session = session().await?;
        let api = api(session.clone());
        Ok(session.with_torrents(|iter| {
            iter.map(|(id, handle)| to_info(&api, id, handle)).collect()
        }))
    }
}

#[cfg(target_arch = "wasm32")]
mod native {
    pub(super) fn unsupported_on_web<T>() -> anyhow::Result<T> {
        anyhow::bail!(
            "Torrent features need real OS sockets, which aren't available on Web. \
             Run this on macOS, Android, iOS, Linux, or Windows instead."
        )
    }
}
