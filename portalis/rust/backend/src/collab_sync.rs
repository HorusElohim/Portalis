//! The manifest-sync mini-protocol — Phase 2 of the collab-collections
//! plan: the one piece of custom networking this backend needs (see
//! `rust/backend/README.md`; everything else rides on librqbit's ordinary
//! BitTorrent machinery). Two devices holding the same invite secret
//! exchange their collection's signed manifest entries and collaborator
//! lists, and each side CRDT-merges what it receives. Peer discovery is
//! *not* handled here yet — Phase 2 uses a manually-entered `ip:port`
//! (shown in the app's debug UI); Phase 3 replaces that with DHT
//! rendezvous, reusing this exact wire protocol unchanged.
//!
//! Wire format: one length-prefixed (4-byte little-endian) JSON
//! [`WireFrame`] each way, over a plain TCP connection we own — librqbit's
//! peer-connection internals are private (verified against its source), so
//! piggybacking on its sockets isn't possible.
//!
//! Collections are identified on the wire by **rendezvous key** (the
//! blake3 hash of the invite secret), never the secret itself — a
//! misdirected or eavesdropped sync request can't leak the ability to
//! join.
//!
//! Not FRB-scanned (not in `tool/frb_build.sh`'s `--rust-input`) for the
//! same visibility reason as `collab_store.rs` / `domain`.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::OnceCell;

use crate::collab_store::{
    self, collaborator_from_persisted, collaborator_to_persisted, entry_from_persisted,
    entry_to_persisted, PersistedCollaborator, PersistedManifestEntry,
};

/// Everything one side knows about one collection. Entries carry their
/// original signatures and are re-verified on receipt (`Manifest::add`),
/// so a malicious peer can inject nothing it couldn't have signed itself.
#[derive(Serialize, Deserialize)]
pub(crate) struct SyncMessage {
    pub(crate) rendezvous_key_hex: String,
    pub(crate) collaborators: Vec<PersistedCollaborator>,
    pub(crate) entries: Vec<PersistedManifestEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum WireFrame {
    /// "Here's my state for this collection" — sent by the initiator, and
    /// echoed back (with the responder's state) on success.
    Sync(SyncMessage),
    /// The responder doesn't hold a collection with that rendezvous key —
    /// i.e. it hasn't joined with the same invite code (yet).
    Unknown,
}

/// Refuse absurd frames rather than allocating whatever a hostile peer
/// claims — a manifest of even thousands of entries is well under this.
const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(15);

async fn write_frame<W: AsyncWriteExt + Unpin>(w: &mut W, frame: &WireFrame) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(frame)?;
    let len = u32::try_from(bytes.len()).context("frame too large to send")?;
    anyhow::ensure!(len <= MAX_FRAME_BYTES, "frame too large to send");
    w.write_all(&len.to_le_bytes()).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}

async fn read_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> anyhow::Result<WireFrame> {
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes).await.context("reading frame length")?;
    let len = u32::from_le_bytes(len_bytes);
    anyhow::ensure!(len <= MAX_FRAME_BYTES, "peer sent an oversized frame");
    let mut bytes = vec![0u8; len as usize];
    r.read_exact(&mut bytes).await.context("reading frame body")?;
    Ok(serde_json::from_slice(&bytes).context("parsing frame")?)
}

/// Snapshot of our local state for the collection with this rendezvous
/// key, or `None` if we don't hold it.
fn local_message_for(rendezvous_key_hex: &str) -> anyhow::Result<Option<SyncMessage>> {
    collab_store::with_store(|collections| {
        Ok(collections
            .iter()
            .find(|c| c.rendezvous_key().to_hex() == rendezvous_key_hex)
            .map(|c| SyncMessage {
                rendezvous_key_hex: rendezvous_key_hex.to_string(),
                collaborators: c.collaborators.iter().map(collaborator_to_persisted).collect(),
                entries: c.manifest().entries().map(entry_to_persisted).collect(),
            }))
    })
}

/// CRDT-merge a peer's state into the matching local collection: manifest
/// entries via `Manifest::add` (signature-verified, duplicate-proof), and
/// collaborators deduplicated by device id. Returns `false` if we don't
/// hold that collection.
fn apply_message(msg: &SyncMessage) -> anyhow::Result<bool> {
    collab_store::with_store(|collections| {
        let Some(collection) = collections
            .iter_mut()
            .find(|c| c.rendezvous_key().to_hex() == msg.rendezvous_key_hex)
        else {
            return Ok(false);
        };
        for e in &msg.entries {
            // Malformed/forged entries are dropped individually rather
            // than failing the whole sync — same tolerance as
            // Manifest::merge.
            if let Ok(entry) = entry_from_persisted(e) {
                collection.add_manifest_entry(entry);
            }
        }
        for c in &msg.collaborators {
            let Ok(collaborator) = collaborator_from_persisted(c) else {
                continue;
            };
            let known = collection
                .collaborators
                .iter()
                .any(|existing| existing.device_id == collaborator.device_id);
            if !known {
                collection.collaborators.push(collaborator);
            }
        }
        Ok(true)
    })
}

static LISTENER: OnceCell<SocketAddr> = OnceCell::const_new();

/// Starts the sync listener (idempotent) and returns its bound address.
/// Binds an ephemeral port on all interfaces; each incoming connection is
/// one complete sync exchange (read theirs → merge → reply with ours).
pub(crate) async fn ensure_listener() -> anyhow::Result<SocketAddr> {
    LISTENER
        .get_or_try_init(|| async {
            let listener = TcpListener::bind(("0.0.0.0", 0))
                .await
                .context("binding sync listener")?;
            let addr = listener.local_addr()?;
            tokio::spawn(async move {
                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        break;
                    };
                    tokio::spawn(async move {
                        // Per-connection errors (bad peer, timeout) are
                        // intentionally dropped — they mustn't kill the
                        // accept loop, and the *initiating* side surfaces
                        // its own error to its user.
                        let _ = handle_incoming(stream).await;
                    });
                }
            });
            Ok(addr)
        })
        .await
        .copied()
}

async fn handle_incoming(mut stream: TcpStream) -> anyhow::Result<()> {
    let frame = tokio::time::timeout(IO_TIMEOUT, read_frame(&mut stream)).await??;
    let WireFrame::Sync(theirs) = frame else {
        anyhow::bail!("unexpected frame from initiator");
    };
    let reply = match local_message_for(&theirs.rendezvous_key_hex)? {
        Some(ours) => {
            apply_message(&theirs)?;
            WireFrame::Sync(ours)
        }
        None => WireFrame::Unknown,
    };
    tokio::time::timeout(IO_TIMEOUT, write_frame(&mut stream, &reply)).await??;
    Ok(())
}

/// One full sync with a peer at `peer_addr`, for the collection with this
/// rendezvous key: send our state, receive theirs, merge. Symmetric — both
/// sides end up with the union.
pub(crate) async fn sync_with(rendezvous_key_hex: &str, peer_addr: &str) -> anyhow::Result<()> {
    let ours = local_message_for(rendezvous_key_hex)?
        .ok_or_else(|| anyhow::anyhow!("no local collection with that rendezvous key"))?;
    let mut stream = tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(peer_addr))
        .await
        .context("connection timed out")?
        .with_context(|| format!("connecting to {peer_addr}"))?;
    tokio::time::timeout(IO_TIMEOUT, write_frame(&mut stream, &WireFrame::Sync(ours))).await??;
    match tokio::time::timeout(IO_TIMEOUT, read_frame(&mut stream)).await?? {
        WireFrame::Sync(theirs) => {
            apply_message(&theirs)?;
            Ok(())
        }
        WireFrame::Unknown => anyhow::bail!(
            "The other device doesn't have this collection — it needs to join \
             with the same invite code first."
        ),
    }
}

/// Best-effort LAN IP for showing a connectable "sync address" in the UI.
/// The connect() never sends a packet (UDP) — it just makes the OS pick
/// the outbound interface, whose address is what peers on the same network
/// can reach us at.
pub(crate) fn lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::DeviceIdentity;
    use crate::domain::manifest::{InfoHash, ManifestEntry};

    #[tokio::test]
    async fn frames_round_trip_over_a_duplex_stream() {
        let identity = DeviceIdentity::generate();
        let entry = ManifestEntry::new_signed(
            InfoHash::from_bytes([9; 20]),
            "sunset.mp4".into(),
            None,
            &identity,
            1_234,
        );
        let sent = WireFrame::Sync(SyncMessage {
            rendezvous_key_hex: "ab".repeat(32),
            collaborators: vec![],
            entries: vec![entry_to_persisted(&entry)],
        });

        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        write_frame(&mut a, &sent).await.unwrap();
        let received = read_frame(&mut b).await.unwrap();

        let WireFrame::Sync(msg) = received else {
            panic!("expected Sync frame");
        };
        assert_eq!(msg.rendezvous_key_hex, "ab".repeat(32));
        assert_eq!(msg.entries.len(), 1);
        // The entry must still verify after the wire round-trip — same
        // property the persistence tests assert for disk.
        assert!(entry_from_persisted(&msg.entries[0]).unwrap().verify());
    }

    #[tokio::test]
    async fn unknown_frame_round_trips() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_frame(&mut a, &WireFrame::Unknown).await.unwrap();

        assert!(matches!(read_frame(&mut b).await.unwrap(), WireFrame::Unknown));
    }

    #[tokio::test]
    async fn oversized_frames_are_rejected_not_allocated() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        // A hostile length prefix claiming ~4GB.
        a.write_all(&u32::MAX.to_le_bytes()).await.unwrap();

        assert!(read_frame(&mut b).await.is_err());
    }
}
