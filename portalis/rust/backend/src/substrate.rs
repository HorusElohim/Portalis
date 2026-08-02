//! What moves the bytes.
//!
//! The collection model says a claim points at content; it does not say
//! BitTorrent. Naming that boundary is what lets `fetch` and `delete` be
//! tested at all — they are the two paths where every remaining bug can hide,
//! and until now exercising either meant a real swarm.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::torrent::{NewFile, TorrentInfo};

#[async_trait]
pub(crate) trait Substrate: Send + Sync {
    /// Make these files available, and answer with the handle others fetch by.
    async fn publish(&self, name: String, files: Vec<NewFile>) -> anyhow::Result<TorrentInfo>;

    /// Start acquiring this handle, with any addresses known to hold it.
    async fn acquire(&self, handle: &str, peers: Vec<SocketAddr>) -> anyhow::Result<TorrentInfo>;

    /// Stop carrying it. Files already on disk stay there.
    async fn release(&self, handle: &str) -> anyhow::Result<()>;

    /// Everything held right now.
    async fn holdings(&self) -> Vec<TorrentInfo>;

    fn ready(&self) -> bool;
}

/// The real one.
pub(crate) struct Torrents;

#[async_trait]
impl Substrate for Torrents {
    async fn publish(&self, name: String, files: Vec<NewFile>) -> anyhow::Result<TorrentInfo> {
        crate::torrent::create_collection(name, files).await
    }

    async fn acquire(&self, handle: &str, peers: Vec<SocketAddr>) -> anyhow::Result<TorrentInfo> {
        crate::torrent::add_info_hash_with_peers(handle, peers).await
    }

    async fn release(&self, handle: &str) -> anyhow::Result<()> {
        crate::torrent::forget_torrent(handle).await
    }

    async fn holdings(&self) -> Vec<TorrentInfo> {
        crate::torrent::list_torrents().await.unwrap_or_default()
    }

    fn ready(&self) -> bool {
        crate::torrent::session_started()
    }
}

static CURRENT: Mutex<Option<Arc<dyn Substrate>>> = Mutex::new(None);

pub(crate) fn current() -> Arc<dyn Substrate> {
    CURRENT
        .lock()
        .unwrap()
        .get_or_insert_with(|| Arc::new(Torrents))
        .clone()
}

/// Swaps in a test double until the returned guard is dropped.
#[cfg(test)]
pub(crate) fn use_double(double: Arc<dyn Substrate>) -> Restore {
    *CURRENT.lock().unwrap() = Some(double);
    Restore
}

#[cfg(test)]
pub(crate) struct Restore;

#[cfg(test)]
impl Drop for Restore {
    fn drop(&mut self) {
        *CURRENT.lock().unwrap() = None;
    }
}

/// Records what was asked of it and answers from a list. Enough to test the
/// two paths that decide *what* to move, which is where the bugs have been —
/// nothing here pretends to move anything.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Recorded {
    pub(crate) held: Mutex<Vec<String>>,
    pub(crate) acquired: Mutex<Vec<String>>,
    pub(crate) released: Mutex<Vec<String>>,
}

#[cfg(test)]
#[async_trait]
impl Substrate for Recorded {
    async fn publish(&self, name: String, _files: Vec<NewFile>) -> anyhow::Result<TorrentInfo> {
        anyhow::bail!("publish is not part of what this double is for ({name})")
    }

    async fn acquire(&self, handle: &str, _peers: Vec<SocketAddr>) -> anyhow::Result<TorrentInfo> {
        self.acquired.lock().unwrap().push(handle.to_string());
        anyhow::bail!("nothing to acquire from")
    }

    async fn release(&self, handle: &str) -> anyhow::Result<()> {
        self.released.lock().unwrap().push(handle.to_string());
        Ok(())
    }

    async fn holdings(&self) -> Vec<TorrentInfo> {
        self.held.lock().unwrap().iter().map(String::as_str).map(held_torrent).collect()
    }

    fn ready(&self) -> bool {
        true
    }
}

#[cfg(test)]
fn held_torrent(info_hash: &str) -> TorrentInfo {
    TorrentInfo {
        id: 0,
        info_hash: info_hash.to_string(),
        name: "held".into(),
        state: "live".into(),
        progress_bytes: 1,
        total_bytes: 1,
        uploaded_bytes: 0,
        download_mbps: 0.0,
        upload_mbps: 0.0,
        finished: true,
        error: None,
        live_peers: 0,
        files: Vec::new(),
    }
}
