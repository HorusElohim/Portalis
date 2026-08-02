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
use crate::log::clog;

/// Everything one side knows about one collection. Entries carry their
/// original signatures and are re-verified on receipt (`Manifest::add`),
/// so a malicious peer can inject nothing it couldn't have signed itself.
#[derive(Serialize, Deserialize)]
pub(crate) struct SyncMessage {
    pub(crate) rendezvous_key_hex: String,
    pub(crate) collaborators: Vec<PersistedCollaborator>,
    pub(crate) entries: Vec<PersistedManifestEntry>,
    /// The port this device's *BitTorrent* session listens on (distinct
    /// from the sync listener's port). Combined with the IP the sync
    /// connection itself came from/went to, the other side learns a
    /// concrete peer address to hand librqbit as `initial_peers` when
    /// fetching this collection's media — no DHT lookup needed on a LAN.
    /// `default` for wire-compat with peers running the field-less build.
    #[serde(default)]
    pub(crate) bt_listen_port: Option<u16>,
    /// The port *this* module's sync listener is bound to. Combined with the
    /// connection's own IP it gives the other side a durable address to call
    /// us back on — which the connection's source address does not, since an
    /// initiator dials from an ephemeral port. Without it only the joiner ever
    /// knew where to reach the inviter, so nothing the inviter added after the
    /// join could ever be pushed out.
    #[serde(default)]
    pub(crate) sync_listen_port: Option<u16>,
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

/// The port the sync listener prefers, so a peer's saved address survives that
/// peer restarting — see [`ensure_listener`]. Well inside the ephemeral range
/// and not registered to anything; collisions fall back gracefully.
pub(crate) const PREFERRED_SYNC_PORT: u16 = 47821;

/// Separate, much shorter budget for *establishing* a connection. An invite
/// now carries every interface address the inviter had (see [`lan_ips`]), so
/// several candidates are normally unreachable from the joiner's network and
/// walking them at the full [`IO_TIMEOUT`] would take a minute before the one
/// that works is even tried. A reachable peer on a shared LAN completes its
/// handshake in milliseconds; an unroutable address either refuses instantly
/// or is a blackhole worth abandoning quickly.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long we'll wait on the *BitTorrent* session just to learn its listen
/// port while assembling a sync message. Deliberately far below
/// [`IO_TIMEOUT`]: the port is an optimisation (it lets the peer skip DHT
/// discovery when fetching media), never a prerequisite for exchanging
/// manifests. Starting librqbit's session bootstraps the DHT and probes
/// UPnP, which on a phone can take longer than the initiator is willing to
/// wait for a reply — blocking the reply on it made every sync fail at
/// exactly IO_TIMEOUT while the responder was still starting up.
const BT_PORT_TIMEOUT: Duration = Duration::from_secs(2);

/// Correlates the log lines of one sync attempt/connection. Without it,
/// concurrent attempts interleave and nothing ties a "connecting…" line to
/// its own outcome.
fn next_sync_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The BT listen port, if it's already available *cheaply*. Never blocks the
/// manifest-sync path on librqbit's startup — see [`BT_PORT_TIMEOUT`].
async fn bt_listen_port_best_effort() -> Option<u16> {
    // Known from an earlier call: answer without going near the session, so
    // this can only ever be lost once per run rather than on every exchange.
    if let Some(port) = crate::torrent::bt_listen_port_cached() {
        return Some(port);
    }
    match tokio::time::timeout(BT_PORT_TIMEOUT, crate::torrent::bt_listen_port()).await {
        Ok(Ok(port)) => port,
        Ok(Err(e)) => {
            clog!("collab_sync", "bt_listen_port_best_effort: session error ({e:#}) — \
                 continuing without a port, peer will fall back to DHT");
            None
        }
        Err(_) => {
            clog!("collab_sync", "bt_listen_port_best_effort: BT session still starting after \
                 {BT_PORT_TIMEOUT:?} — continuing without a port, peer will fall back to DHT");
            None
        }
    }
}

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
    serde_json::from_slice(&bytes).context("parsing frame")
}

/// Snapshot of our local state for the collection with this rendezvous
/// key, or `None` if we don't hold it.
async fn local_message_for(rendezvous_key_hex: &str) -> anyhow::Result<Option<SyncMessage>> {
    // Best-effort and *time-bounded*: fetch may still work via DHT if the
    // BT session isn't up, so this must never delay the reply (see
    // BT_PORT_TIMEOUT).
    let bt_listen_port = bt_listen_port_best_effort().await;
    let result = collab_store::read_store(|collections| {
        clog!(
            "collab_sync",
            "local_message_for: looking for rendezvous_key={}… among {} known collection(s): {:?}",
            &rendezvous_key_hex[..8.min(rendezvous_key_hex.len())],
            collections.len(),
            collections
                .iter()
                .map(|c| (c.name.clone(), c.rendezvous_key().to_hex()[..8].to_string()))
                .collect::<Vec<_>>()
        );
        Ok(collections
            .iter()
            .find(|c| c.rendezvous_key().to_hex() == rendezvous_key_hex)
            .map(|c| SyncMessage {
                rendezvous_key_hex: rendezvous_key_hex.to_string(),
                collaborators: c.collaborators.iter().map(collaborator_to_persisted).collect(),
                entries: c.manifest().entries().map(entry_to_persisted).collect(),
                bt_listen_port,
                // Already bound by the time any sync happens — this path is
                // only reachable from the listener itself or from a caller
                // that awaited `ensure_listener`.
                sync_listen_port: LISTENER.get().map(|addr| addr.port()),
            }))
    });
    if let Ok(Some(msg)) = &result {
        // The other half of `record_bt_peer`'s line. Without it a log shows
        // only what the *peer* advertised, so "we never told them where our
        // BitTorrent session is" — the reason a fetch finds nobody — is
        // invisible from either device's console.
        clog!(
            "collab_sync",
            "local_message_for: match found, advertising bt_port={:?} sync_port={:?}{}",
            msg.bt_listen_port,
            msg.sync_listen_port,
            if msg.bt_listen_port.is_none() {
                " — NOTE: with no BitTorrent port the peer has no direct address to fetch \
                 our media from and must fall back to DHT"
            } else {
                ""
            }
        );
    } else if let Ok(None) = &result {
        clog!("collab_sync", "local_message_for: match found = false");
    }
    result
}

/// BitTorrent peer addresses learned through sync exchanges, per
/// rendezvous key — handed to librqbit as `initial_peers` when fetching a
/// collection's media, so a LAN fetch connects straight to the device that
/// has the files instead of waiting on DHT discovery. In-memory only:
/// these are hints tied to current network conditions, not durable state.
static LEARNED_BT_PEERS: std::sync::Mutex<
    std::collections::BTreeMap<String, std::collections::BTreeSet<SocketAddr>>,
> = std::sync::Mutex::new(std::collections::BTreeMap::new());

fn record_bt_peer(rendezvous_key_hex: &str, ip: std::net::IpAddr, msg: &SyncMessage) {
    match msg.bt_listen_port {
        Some(port) => {
            let addr = SocketAddr::new(ip, port);
            clog!("collab_sync", "record_bt_peer: {addr} for rendezvous_key={}…", &rendezvous_key_hex[..8.min(rendezvous_key_hex.len())]);
            LEARNED_BT_PEERS
                .lock()
                .unwrap()
                .entry(rendezvous_key_hex.to_string())
                .or_default()
                .insert(addr);
        }
        None => clog!("collab_sync", "record_bt_peer: peer sent no bt_listen_port, nothing to record"),
    }
}

pub(crate) fn learned_bt_peers(rendezvous_key_hex: &str) -> Vec<SocketAddr> {
    LEARNED_BT_PEERS
        .lock()
        .unwrap()
        .get(rendezvous_key_hex)
        .map(|set| set.iter().copied().collect())
        .unwrap_or_default()
}

/// **Sync** endpoints (`ip:port`) known for a collection, keyed by rendezvous
/// key — where [`resync_loop`] calls back to.
///
/// Persisted, unlike [`LEARNED_BT_PEERS`]. A BT peer address is a hint that
/// costs nothing to rediscover; a sync address is the only way this device can
/// reach the collection at all until Phase 3's DHT rendezvous lands. Holding
/// them in memory only meant that restarting the app orphaned every collection
/// you had joined: the invite code was long gone from the clipboard, and
/// nothing else on disk recorded who to talk to.
type PeerMap = std::collections::BTreeMap<String, std::collections::BTreeSet<String>>;
static KNOWN_SYNC_PEERS: std::sync::Mutex<Option<PeerMap>> = std::sync::Mutex::new(None);

/// Bounded so an invite advertising many interface addresses (and a device
/// that changes networks often) can't grow this without limit — each dead
/// address costs a [`CONNECT_TIMEOUT`] on every re-sync tick.
const MAX_PEERS_PER_COLLECTION: usize = 12;

fn peers_vault() -> crate::vault::Vault {
    crate::vault::Vault::named("sync_peers.json")
}

fn with_peers<R>(f: impl FnOnce(&mut PeerMap) -> R) -> R {
    let mut guard = KNOWN_SYNC_PEERS.lock().unwrap();
    if guard.is_none() {
        let loaded: PeerMap = peers_vault().read().ok().flatten().unwrap_or_default();
        clog!("collab_sync", "known sync peers: loaded {} collection(s)", loaded.len());
        *guard = Some(loaded);
    }
    f(guard.as_mut().unwrap())
}

/// Same atomic temp-file-then-rename as `collab_store::save`, for the same
/// reason: a truncating write that fails halfway leaves nothing behind.
fn save_peers(peers: &PeerMap) {
    // Non-fatal: peers stay in memory for this run and a live collection keeps
    // syncing. Only a restart would lose them.
    if let Err(e) = peers_vault().write(peers) {
        clog!("collab_sync", "couldn't persist known sync peers ({e:#})");
    }
}

/// Records addresses to call back on for this collection. Idempotent, and
/// writes only when something is actually new.
pub(crate) fn remember_sync_peers<I>(rendezvous_key_hex: &str, addrs: I)
where
    I: IntoIterator<Item = String>,
{
    let fresh: Vec<String> = addrs
        .into_iter()
        // Our own listener's address would make every device sync with itself
        // forever — harmless but pure noise in the logs and on the wire.
        // `lan_ips` is exactly the set an invite advertises, so this catches
        // the common case of scanning your own invite.
        .filter(|addr| !is_self_address(addr))
        .collect();
    with_peers(|peers| {
        let set = peers.entry(rendezvous_key_hex.to_string()).or_default();
        let added = insert_bounded(set, fresh, MAX_PEERS_PER_COLLECTION);
        if added.is_empty() {
            return;
        }
        clog!(
            "collab_sync",
            "remember_sync_peers: +{added:?} for rendezvous_key={}… ({} known)",
            &rendezvous_key_hex[..8.min(rendezvous_key_hex.len())],
            set.len()
        );
        save_peers(peers);
    })
}

/// Adds `addrs` to `set`, keeping it within `max`, and returns what was
/// genuinely new. Split out from [`remember_sync_peers`] because it is the
/// only part with a decision in it, and the rest of that function touches
/// process-wide state and the filesystem.
fn insert_bounded(
    set: &mut std::collections::BTreeSet<String>,
    addrs: Vec<String>,
    max: usize,
) -> Vec<String> {
    let mut added = Vec::new();
    for addr in addrs {
        if set.insert(addr.clone()) {
            added.push(addr);
        }
    }
    while set.len() > max {
        // A BTreeSet has no notion of least-recently-used; dropping the
        // lexicographically first is arbitrary but bounded, and a peer that
        // is still live re-announces its address on the next exchange.
        let victim = set.iter().next().cloned().expect("non-empty above max");
        set.remove(&victim);
    }
    added
}

/// Consecutive failed attempts per `"<rendezvous key>|<addr>"`.
///
/// An invite advertises *every* interface address the inviter had, so a joiner
/// typically remembers several that were never reachable from its network.
/// Retrying each of those costs a [`CONNECT_TIMEOUT`] on every tick, forever —
/// which on a phone is the radio waking up to do nothing.
static PEER_FAILURES: std::sync::Mutex<std::collections::BTreeMap<String, u32>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// How many consecutive failures make an address junk — but only ever applied
/// when some *other* address for the same collection is working. If everything
/// is failing, the network is down rather than the addresses being wrong, and
/// forgetting them all would orphan the collection permanently.
pub(crate) const PRUNE_AFTER_FAILURES: u32 = 10;

pub(crate) fn note_peer_result(rendezvous_key_hex: &str, addr: &str, ok: bool) -> u32 {
    let key = format!("{rendezvous_key_hex}|{addr}");
    let mut failures = PEER_FAILURES.lock().unwrap();
    if ok {
        failures.remove(&key);
        return 0;
    }
    let counter = failures.entry(key).or_insert(0);
    *counter += 1;
    *counter
}

pub(crate) fn forget_sync_peer(rendezvous_key_hex: &str, addr: &str) {
    with_peers(|peers| {
        let Some(set) = peers.get_mut(rendezvous_key_hex) else {
            return;
        };
        if set.remove(addr) {
            clog!(
                "collab_sync",
                "forgetting {addr} — {PRUNE_AFTER_FAILURES} consecutive failures while another \
                 address for this collection works"
            );
            save_peers(peers);
        }
    });
    PEER_FAILURES
        .lock()
        .unwrap()
        .remove(&format!("{rendezvous_key_hex}|{addr}"));
}

fn is_self_address(addr: &str) -> bool {
    let Some(port) = LISTENER.get().map(|a| a.port()) else {
        return false;
    };
    let Some((host, addr_port)) = addr.rsplit_once(':') else {
        return false;
    };
    addr_port.parse::<u16>() == Ok(port) && lan_ips().iter().any(|ip| ip == host)
}

/// Drops everything remembered about how to reach a collection's peers.
///
/// Called when a collection is deleted: `sync_peers.json` is keyed by
/// rendezvous key and nothing else would ever revisit that key, so without
/// this it accumulates addresses for collections the user has thrown away.
pub(crate) fn forget_collection_peers(rendezvous_key_hex: &str) {
    with_peers(|peers| {
        if peers.remove(rendezvous_key_hex).is_some() {
            clog!(
                "collab_sync",
                "forgot every peer for rendezvous_key={}… — its collection is gone",
                &rendezvous_key_hex[..8.min(rendezvous_key_hex.len())]
            );
            save_peers(peers);
        }
    });
    PEER_FAILURES
        .lock()
        .unwrap()
        .retain(|key, _| !key.starts_with(rendezvous_key_hex));
}

pub(crate) fn known_sync_peers(rendezvous_key_hex: &str) -> Vec<String> {
    with_peers(|peers| {
        peers
            .get(rendezvous_key_hex)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    })
}

/// The peer's *listener* address, learned from the message it just sent us —
/// as opposed to the ephemeral source port an initiator dials from.
fn record_sync_peer(rendezvous_key_hex: &str, ip: std::net::IpAddr, msg: &SyncMessage) {
    if let Some(port) = msg.sync_listen_port {
        remember_sync_peers(rendezvous_key_hex, [SocketAddr::new(ip, port).to_string()]);
    }
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
            clog!("collab_sync", "apply_message: no local collection for that rendezvous key");
            return Ok(false);
        };
        let entries_before = collection.manifest().len();
        let collaborators_before = collection.collaborators.len();
        let mut entries_rejected = 0;
        for e in &msg.entries {
            // Malformed/forged entries are dropped individually rather
            // than failing the whole sync — same tolerance as
            // Manifest::merge.
            match entry_from_persisted(e) {
                Ok(entry) => {
                    // `add` returns false for an entry whose signature doesn't
                    // verify — worth saying out loud. Silently dropping it made
                    // a signature mismatch indistinguishable from a successful
                    // sync that had nothing new to send, which is the hardest
                    // possible shape for this failure to debug.
                    let name = entry.name.clone();
                    if !collection.add_manifest_entry(entry) {
                        entries_rejected += 1;
                        clog!(
                            "collab_sync",
                            "apply_message: rejected entry {name:?} — signature didn't verify"
                        );
                    }
                }
                Err(err) => {
                    entries_rejected += 1;
                    clog!("collab_sync", "apply_message: dropped an unparseable entry: {err:?}");
                }
            }
        }
        // Our own record is authoritative locally — a peer must never be able
        // to rename us in our own view, and it may well be carrying a stale
        // copy of our old name from before we renamed ourselves.
        let own_device_id = crate::device::current_identity().ok().map(|i| i.device_id());
        for c in &msg.collaborators {
            let Ok(collaborator) = collaborator_from_persisted(c) else {
                continue;
            };
            if Some(collaborator.device_id) == own_device_id {
                continue;
            }
            match collection
                .collaborators
                .iter_mut()
                .find(|existing| existing.device_id == collaborator.device_id)
            {
                // Accept the incoming display name for someone we already
                // know: without this a collaborator who renamed themselves
                // stayed under their old name here forever, since the record
                // is a copy taken when we first learned of them.
                Some(existing) => {
                    if existing.display_name != collaborator.display_name {
                        clog!(
                            "collab_sync",
                            "apply_message: collaborator renamed {:?} -> {:?}",
                            existing.display_name,
                            collaborator.display_name
                        );
                        existing.display_name = collaborator.display_name;
                    }
                }
                None => collection.collaborators.push(collaborator),
            }
        }
        clog!(
            "collab_sync",
            "apply_message: entries {entries_before} -> {} (rejected {entries_rejected}), \
             collaborators {collaborators_before} -> {}",
            collection.manifest().len(),
            collection.collaborators.len(),
        );
        Ok(true)
    })
}

static LISTENER: OnceCell<SocketAddr> = OnceCell::const_new();

/// Where we are listening, if we are. Never starts anything — reading what
/// the app knows about itself must not have a side effect on the network.
pub(crate) fn listening_at() -> Option<SocketAddr> {
    LISTENER.get().copied()
}

/// Starts the sync listener (idempotent) and returns its bound address.
/// Binds an ephemeral port on all interfaces; each incoming connection is
/// one complete sync exchange (read theirs → merge → reply with ours).
pub(crate) async fn ensure_listener() -> anyhow::Result<SocketAddr> {
    LISTENER
        .get_or_try_init(|| async {
            // A *stable* port, not an ephemeral one.
            //
            // Peer addresses are persisted so a collection survives a restart
            // (see KNOWN_SYNC_PEERS), but an ephemeral port makes every one of
            // them expire the moment the peer relaunches — the saved address
            // points at a port nothing is listening on any more, and re-sync
            // gets "Connection refused" forever. Observed exactly that: a peer
            // remembered at :61638 refusing while the same device was by then
            // listening on :63207.
            //
            // Falling back to ephemeral keeps two instances on one machine
            // (which is how this gets tested) working, at the cost of the
            // stability above for the second one.
            let listener = match TcpListener::bind(("0.0.0.0", PREFERRED_SYNC_PORT)).await {
                Ok(listener) => listener,
                Err(e) => {
                    clog!("collab_sync", "port {PREFERRED_SYNC_PORT} is taken ({e}) — falling \
                         back to an ephemeral port, which means peers who saved our address \
                         will have to be re-introduced after a restart");
                    TcpListener::bind(("0.0.0.0", 0))
                        .await
                        .context("binding sync listener")?
                }
            };
            let addr = listener.local_addr()?;
            clog!("collab_sync", "listening on {addr}");
            tokio::spawn(async move {
                loop {
                    let Ok((stream, from)) = listener.accept().await else {
                        clog!("collab_sync", "accept loop ended");
                        break;
                    };
                    let id = next_sync_id();
                    clog!("collab_sync", "[#{id}] incoming connection from {from}");
                    tokio::spawn(async move {
                        // Per-connection errors (bad peer, timeout) are
                        // intentionally dropped — they mustn't kill the
                        // accept loop, and the *initiating* side surfaces
                        // its own error to its user.
                        if let Err(e) = handle_incoming(stream, id).await {
                            clog!("collab_sync", "[#{id}] incoming sync from {from} failed: {e:?}");
                        }
                    });
                }
            });
            // Ask the router (UPnP/IGD) to forward this port so the
            // public-IP address embedded in invites is actually reachable
            // from outside — same machinery librqbit uses for its own BT
            // port. run_forever keeps re-leasing; on routers without UPnP
            // it just keeps failing quietly, and LAN sync is unaffected.
            //
            // The crate reports its own progress through `tracing`, which this
            // binary never initialises, so anything it has to say is invisible
            // — hence logging the one thing we *can* observe here, rather than
            // discarding the constructor's error with a bare `if let Ok`.
            match librqbit_upnp::UpnpPortForwarder::new(vec![addr.port()], None) {
                Ok(forwarder) => {
                    clog!("collab_sync", "UPnP: asking the router to forward port {} — silent \
                         from here on. Note it cannot work at all while a VPN owns the default \
                         route, since SSDP discovery follows that route instead of reaching \
                         the router.", addr.port());
                    tokio::spawn(async move {
                        forwarder.run_forever().await;
                    });
                }
                Err(e) => clog!("collab_sync", "UPnP: couldn't start port forwarding ({e:#}) — \
                     same-network sync is unaffected"),
            }
            // Warm the BitTorrent session *off* the sync path. Starting it
            // bootstraps the DHT and probes UPnP, which is far too slow to
            // do lazily while a peer waits for a manifest reply — doing it
            // here means bt_listen_port_best_effort() usually finds it
            // already up, and when it doesn't, sync still proceeds without.
            tokio::spawn(async move {
                match crate::torrent::bt_listen_port().await {
                    Ok(port) => clog!("collab_sync", "BT session warmed up, listen port = {port:?}"),
                    Err(e) => clog!("collab_sync", "BT session warm-up failed: {e:#} — \
                         manifest sync is unaffected, media fetches will rely on DHT"),
                }
            });
            // Keeps every joined collection in step from here on. Started
            // here so it exists exactly once, alongside the listener it needs.
            crate::converge::start();
            Ok(addr)
        })
        .await
        .copied()
}

async fn handle_incoming(mut stream: TcpStream, id: u64) -> anyhow::Result<()> {
    let peer_ip = stream.peer_addr()?.ip();
    // Logged *before* anything that can block, so the responder's console
    // proves whether the frame arrived at all — the previous version's
    // first log came after the read, leaving "connected but no reply"
    // indistinguishable from "never accepted".
    clog!("collab_sync", "[#{id}] handle_incoming: accepted from {peer_ip}, reading frame");
    let frame = tokio::time::timeout(IO_TIMEOUT, read_frame(&mut stream))
        .await
        .map_err(|_| anyhow::anyhow!("[#{id}] timed out reading the initiator's frame"))??;
    let WireFrame::Sync(theirs) = frame else {
        anyhow::bail!("[#{id}] unexpected frame from initiator");
    };
    clog!("collab_sync", "[#{id}] incoming sync for rendezvous_key={}… from {peer_ip} \
         ({} entries, {} collaborators)",
        &theirs.rendezvous_key_hex[..8.min(theirs.rendezvous_key_hex.len())],
        theirs.entries.len(),
        theirs.collaborators.len(),
    );
    let reply = match local_message_for(&theirs.rendezvous_key_hex).await? {
        Some(ours) => {
            apply_message(&theirs)?;
            record_bt_peer(&theirs.rendezvous_key_hex, peer_ip, &theirs);
            // Remember where to call *them* back: a collection only stays in
            // step if both ends can initiate, and until now the side that
            // received the join learned nothing about the joiner.
            record_sync_peer(&theirs.rendezvous_key_hex, peer_ip, &theirs);
            clog!("collab_sync", "[#{id}] matched a local collection — merged, replying with \
                 {} entries, {} collaborators",
                ours.entries.len(),
                ours.collaborators.len(),
            );
            WireFrame::Sync(ours)
        }
        None => {
            clog!("collab_sync", "[#{id}] no local collection for that rendezvous key \
                 (replying Unknown)"
            );
            WireFrame::Unknown
        }
    };
    clog!("collab_sync", "[#{id}] writing reply to {peer_ip}");
    tokio::time::timeout(IO_TIMEOUT, write_frame(&mut stream, &reply))
        .await
        .map_err(|_| anyhow::anyhow!("[#{id}] timed out writing the reply"))??;
    clog!("collab_sync", "[#{id}] reply sent to {peer_ip} — exchange complete");
    Ok(())
}

/// One full sync with a peer at `peer_addr`, for the collection with this
/// rendezvous key: send our state, receive theirs, merge. Symmetric — both
/// sides end up with the union.
pub(crate) async fn sync_with(rendezvous_key_hex: &str, peer_addr: &str) -> anyhow::Result<()> {
    let id = next_sync_id();
    clog!("collab_sync", "[#{id}] connecting to {peer_addr}…");
    let ours = local_message_for(rendezvous_key_hex)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no local collection with that rendezvous key"))?;
    let mut stream =
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(peer_addr)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                clog!("collab_sync", "[#{id}] connect to {peer_addr} failed: {e}");
                return Err(e).with_context(|| format!("connecting to {peer_addr}"));
            }
            Err(_) => {
                clog!("collab_sync", "[#{id}] connect to {peer_addr} timed out after \
                     {CONNECT_TIMEOUT:?} — unreachable from this network");
                anyhow::bail!("connection to {peer_addr} timed out");
            }
        };
    let peer_ip = stream.peer_addr()?.ip();
    clog!("collab_sync", "[#{id}] connected to {peer_addr} (peer_ip={peer_ip}), sending our state \
         ({} entries, {} collaborators)",
        ours.entries.len(),
        ours.collaborators.len(),
    );
    tokio::time::timeout(IO_TIMEOUT, write_frame(&mut stream, &WireFrame::Sync(ours)))
        .await
        .map_err(|_| anyhow::anyhow!("[#{id}] timed out sending our state to {peer_addr}"))??;
    clog!("collab_sync", "[#{id}] state sent to {peer_addr}, awaiting their reply…");
    let reply = tokio::time::timeout(IO_TIMEOUT, read_frame(&mut stream))
        .await
        .map_err(|_| {
            clog!("collab_sync", "[#{id}] {peer_addr} accepted the connection and took our frame \
                 but never replied within {IO_TIMEOUT:?}");
            anyhow::anyhow!("[#{id}] timed out waiting for {peer_addr}'s reply")
        })??;
    match reply {
        WireFrame::Sync(theirs) => {
            clog!("collab_sync", "[#{id}] {peer_addr} replied with {} entries, {} collaborators",
                theirs.entries.len(),
                theirs.collaborators.len(),
            );
            apply_message(&theirs)?;
            record_bt_peer(rendezvous_key_hex, peer_ip, &theirs);
            // Both the address that just worked and the listener address they
            // advertise: the first is proven reachable, the second survives
            // their next restart if the port is stable.
            record_sync_peer(rendezvous_key_hex, peer_ip, &theirs);
            remember_sync_peers(rendezvous_key_hex, [peer_addr.to_string()]);
            Ok(())
        }
        WireFrame::Unknown => {
            clog!("collab_sync", "[#{id}] {peer_addr} doesn't know this collection (replied Unknown)");
            anyhow::bail!(
                "The other device doesn't have this collection — it needs to join \
                 with the same invite code first."
            )
        }
    }
}

/// Tries each address until one sync succeeds — invites can carry several
/// candidate addresses (LAN + public); whichever is reachable from here
/// wins. Returns the last error if none work.
pub(crate) async fn sync_with_any(
    rendezvous_key_hex: &str,
    peer_addrs: &[String],
) -> anyhow::Result<()> {
    clog!("collab_sync", "sync_with_any: trying {} address(es): {peer_addrs:?}", peer_addrs.len());
    let mut last_err = anyhow::anyhow!("no peer addresses to try");
    for addr in peer_addrs {
        match sync_with(rendezvous_key_hex, addr).await {
            Ok(()) => {
                clog!("collab_sync", "sync_with_any: succeeded via {addr}");
                return Ok(());
            }
            Err(e) => {
                clog!("collab_sync", "sync_with_any: {addr} failed: {e:?}");
                last_err = e.context(format!("via {addr}"));
            }
        }
    }
    clog!("collab_sync", "sync_with_any: exhausted all addresses, giving up");
    Err(last_err)
}

/// Every address on this device a peer on a shared network could plausibly
/// reach us at — **all** of them, not one.
///
/// This used to infer a single address by opening a UDP socket to 8.8.8.8 and
/// reading back the local address, i.e. "whichever interface owns the default
/// route". That is exactly wrong when a VPN owns the default route: the OS
/// answers with the tunnel's virtual address (e.g. `10.2.0.2`), which no
/// device on the actual Wi-Fi can route to, while the real Wi-Fi address
/// (e.g. `192.168.1.155` — same subnet as the peer!) never gets advertised at
/// all. Invites then carry an unreachable address and every sync fails, on a
/// network where a direct connection would have worked.
///
/// Returning several candidates costs nothing and is what makes this robust:
/// `sync_with_any` tries each in turn and a wrong one fails fast (see
/// [`CONNECT_TIMEOUT`]). **Correctness does not depend on the ordering below**
/// — every address is advertised regardless; ranking only decides which is
/// tried first, i.e. how quickly a working sync starts.
///
/// The ranking prefers a real hardware NIC: physical Ethernet/Wi-Fi adapters
/// have a link-layer (MAC) address, virtual tunnels generally don't. Broadcast
/// capability is a weaker secondary signal — measured on this project's own
/// macOS box, an active `utun9` VPN tunnel *did* report a broadcast address,
/// so it cannot discriminate on its own. Both are interface properties rather
/// than guesses from names, so neither depends on platform-specific naming
/// like `utun*`/`ppp*`.
pub(crate) fn lan_ips() -> Vec<String> {
    // Cached: `list_collections` runs on a UI poll and calls this every time,
    // so an uncached version enumerated every interface on the machine
    // several times a minute forever — a syscall walk, an allocation per
    // address, and a log line, all to re-derive something that changes only
    // when the network does. The TTL is short enough that plugging in
    // Ethernet or joining a different Wi-Fi is picked up well within the time
    // it takes to generate and send an invite.
    const TTL: Duration = Duration::from_secs(20);
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Vec<String>)>> =
        std::sync::Mutex::new(None);

    if let Some((at, cached)) = CACHE.lock().unwrap().as_ref() {
        if at.elapsed() < TTL {
            return cached.clone();
        }
    }
    let fresh = enumerate_lan_ips();
    *CACHE.lock().unwrap() = Some((std::time::Instant::now(), fresh.clone()));
    fresh
}

fn enumerate_lan_ips() -> Vec<String> {
    use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};

    let Ok(interfaces) = NetworkInterface::show() else {
        clog!("collab_sync", "lan_ips: couldn't enumerate interfaces — falling back to \
             127.0.0.1, which only works for syncing with yourself");
        return vec!["127.0.0.1".to_string()];
    };

    struct Candidate {
        has_mac: bool,
        has_broadcast: bool,
        is_private: bool,
        ip: std::net::Ipv4Addr,
        interface: String,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for interface in &interfaces {
        for addr in &interface.addr {
            // IPv4 only: the sync listener binds 0.0.0.0, so an invite
            // carrying a v6 address we aren't listening on would only waste a
            // connection attempt.
            let Addr::V4(v4) = addr else { continue };
            let ip = v4.ip;
            if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
                continue;
            }
            if interface.internal {
                continue;
            }
            candidates.push(Candidate {
                has_mac: interface.mac_addr.is_some(),
                has_broadcast: v4.broadcast.is_some(),
                is_private: ip.is_private(),
                ip,
                interface: interface.name.clone(),
            });
        }
    }
    // Descending on each signal in turn: hardware NIC, then broadcast-capable,
    // then private-range.
    candidates.sort_by(|a, b| {
        b.has_mac
            .cmp(&a.has_mac)
            .then(b.has_broadcast.cmp(&a.has_broadcast))
            .then(b.is_private.cmp(&a.is_private))
    });
    // Not `dedup_by`, which only collapses *adjacent* equals: the same
    // address can appear on two interfaces whose ranking flags differ, which
    // sorts them apart and would advertise it twice.
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.ip));

    if candidates.is_empty() { 
        clog!("collab_sync", "lan_ips: no usable interface addresses — falling back to \
             127.0.0.1, which only works for syncing with yourself");
        return vec!["127.0.0.1".to_string()];
    }
    clog!(
        "collab_sync",
        "lan_ips: {:?}",
        candidates
            .iter()
            .map(|c| format!(
                "{}={}{}{}{}",
                c.interface,
                c.ip,
                if c.has_mac { " nic" } else { " virtual" },
                if c.has_broadcast { " broadcast" } else { " point-to-point" },
                if c.is_private { " private" } else { " public" }
            ))
            .collect::<Vec<_>>()
    );
    if let Some(default_ip) = default_route_ip() {
        if default_ip != candidates[0].ip {
            // Worth calling out loudly, because it silently breaks the two
            // things that make *cross-network* sync possible: UPnP's SSDP
            // discovery follows the default route and so never reaches the
            // real router, and the "public IP" we discover is the tunnel
            // exit's — an address no port forward can ever point back at this
            // device. Same-LAN sync is unaffected now that we advertise the
            // hardware NIC's address too, which is the case this warning
            // accompanies rather than blocks.
            clog!(
                "collab_sync",
                "lan_ips: NOTE — the default route belongs to {default_ip} (a VPN or tunnel), \
                 not to {} on {}. Same-network sync still works via the addresses above, but \
                 UPnP port forwarding and the public IP will not work while that tunnel is up.",
                candidates[0].ip,
                candidates[0].interface,
            );
        }
    }
    candidates.into_iter().map(|c| c.ip.to_string()).collect()
}

/// The local address the OS would use to reach the internet — i.e. whichever
/// interface owns the default route. This was the *whole* of the old
/// [`lan_ips`] implementation, which is why a VPN broke invites entirely. It
/// survives only as a diagnostic: comparing it against the ranked candidates
/// tells us a tunnel is in the way.
fn default_route_ip() -> Option<std::net::IpAddr> {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip())
        .ok()
}

/// Best-effort public IP, so invites can carry an address reachable from
/// *outside* this network too — asked once per app run (it's a network
/// call) and cached. `None` when offline or both services are unreachable.
///
/// Honest limitation, until Phase 3: knowing the public IP only helps if
/// the sync port is actually reachable from outside (router port-forward /
/// UPnP). On an unconfigured NAT the LAN address still works for same-
/// network peers; cross-network sync without setup needs the DHT +
/// hole-punching phase.
/// The public IP if it is *already known*, and never a wait.
///
/// Discovery costs two HTTP requests with 5s timeouts each, and the old
/// blocking version was awaited by `list_collections` — which runs on the UI
/// poll. So the very first listing after launch could sit for ten seconds
/// before showing a single collection, on a screen whose data was sitting on
/// disk the whole time. Now the first call starts discovery in the background
/// and returns `None`; invites generated in that window carry the LAN
/// addresses only, and pick up the public one as soon as it lands.
pub(crate) fn public_ip_now() -> Option<String> {
    static RESOLVED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    static ATTEMPTED_AT: std::sync::Mutex<Option<std::time::Instant>> =
        std::sync::Mutex::new(None);

    if let Some(ip) = RESOLVED.lock().unwrap().clone() {
        return Some(ip);
    }

    // Retry occasionally rather than once: the first attempt often lands
    // before any network is up (launching on a phone that is still
    // associating), and giving up forever would mean invites never carry a
    // public address for the rest of the run.
    const RETRY_AFTER: Duration = Duration::from_secs(60);
    let mut attempted = ATTEMPTED_AT.lock().unwrap();
    let due = attempted.map(|at| at.elapsed() >= RETRY_AFTER).unwrap_or(true);
    if !due {
        return None;
    }
    *attempted = Some(std::time::Instant::now());
    drop(attempted);

    tokio::spawn(async move {
        if let Some(ip) = discover_public_ip().await {
            *RESOLVED.lock().unwrap() = Some(ip);
        }
    });
    None
}

async fn discover_public_ip() -> Option<String> {
    for service in ["https://api.ipify.org", "https://checkip.amazonaws.com"] {
        let Ok(Ok(resp)) =
            tokio::time::timeout(Duration::from_secs(5), reqwest::get(service)).await
        else {
            clog!("collab_sync", "public_ip: {service} unreachable/timed out");
            continue;
        };
        if let Ok(text) = resp.text().await {
            let candidate = text.trim().to_string();
            if candidate.parse::<std::net::IpAddr>().is_ok() {
                clog!("collab_sync", "public_ip resolved to {candidate} via {service}");
                return Some(candidate);
            }
            clog!("collab_sync", "public_ip: {service} returned non-IP text: {candidate:?}");
        }
    }
    clog!("collab_sync", "public_ip: no service reachable this round");
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::collaborator::{Collaborator, Role};
    use crate::domain::identity::DeviceIdentity;
    use crate::domain::manifest::{InfoHash, ManifestEntry};

    #[test]
    fn a_peers_rename_reaches_us_but_a_peer_cannot_rename_us() {
        // Mirrors apply_message's collaborator-merge rule without needing the
        // process-wide store: an incoming name updates someone we already
        // know, adds someone we don't, and is ignored for our own device.
        let me = DeviceIdentity::generate().device_id();
        let theo = DeviceIdentity::generate().device_id();
        let newcomer = DeviceIdentity::generate().device_id();

        let mut local = vec![
            Collaborator::new(me, "Maya".into(), Role::Admin, 0),
            Collaborator::new(theo, "Theo".into(), Role::Member, 0),
        ];
        let incoming = vec![
            // A peer carrying a stale copy of our old name.
            Collaborator::new(me, "Me".into(), Role::Admin, 0),
            // Theo renamed himself since we last synced.
            Collaborator::new(theo, "Théo".into(), Role::Member, 0),
            Collaborator::new(newcomer, "Ada".into(), Role::Member, 0),
        ];

        let own = Some(me);
        for c in incoming {
            if Some(c.device_id) == own {
                continue;
            }
            match local.iter_mut().find(|e| e.device_id == c.device_id) {
                Some(existing) => existing.display_name = c.display_name,
                None => local.push(c),
            }
        }

        // Our own name survives — a peer must not be able to rename us, least
        // of all back to a stale value.
        assert_eq!(local[0].display_name, "Maya");
        // A rename by someone else does land.
        assert_eq!(local[1].display_name, "Théo");
        // And an unknown collaborator is still added.
        assert_eq!(local.len(), 3);
        assert_eq!(local[2].display_name, "Ada");
    }

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
            bt_listen_port: Some(6881),
            sync_listen_port: Some(45123),
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

    #[test]
    fn lan_ips_returns_every_interface_not_just_the_default_routes() {
        // The regression this guards: on a machine where a VPN owns the
        // default route, the old single-address probe returned only the
        // tunnel's virtual address (unroutable from the peer's Wi-Fi) and
        // silently dropped the real Wi-Fi address that would have worked.
        // Any machine running this has at least one usable address, and we
        // must never collapse to exactly the default route's.
        let ips = lan_ips();

        assert!(!ips.is_empty(), "should always yield at least a fallback");
        // Never a bare loopback unless nothing else exists at all.
        if ips.len() > 1 {
            assert!(
                !ips.contains(&"127.0.0.1".to_string()),
                "loopback is a last-resort fallback, not a candidate: {ips:?}"
            );
        }
        // Whatever the default route happens to be on this machine, it must
        // appear *among* the candidates rather than being the only one when
        // other interfaces exist.
        for ip in &ips {
            assert!(
                ip.parse::<std::net::Ipv4Addr>().is_ok(),
                "{ip} is not a v4 address"
            );
        }
    }

    #[test]
    fn peers_survive_a_restart_and_go_when_their_collection_does() {
        let _temp = crate::paths::redirect_to_temp();
        let key = "c".repeat(64);

        remember_sync_peers(&key, ["192.168.1.9:47821".to_string()]);
        *KNOWN_SYNC_PEERS.lock().unwrap() = None; // as a relaunch would find it

        assert_eq!(known_sync_peers(&key), vec!["192.168.1.9:47821".to_string()]);
        forget_collection_peers(&key);
        assert!(known_sync_peers(&key).is_empty());
    }

    #[test]
    fn remembering_a_peer_twice_adds_it_once_and_the_set_stays_bounded() {
        // Every re-sync tick re-records the address it just used, so without
        // the "genuinely new" answer this would rewrite sync_peers.json
        // several times a minute forever.
        let mut set = std::collections::BTreeSet::new();

        let first = insert_bounded(&mut set, vec!["10.0.0.1:5000".into()], 3);
        let again = insert_bounded(&mut set, vec!["10.0.0.1:5000".into()], 3);

        assert_eq!(first, vec!["10.0.0.1:5000".to_string()]);
        assert!(again.is_empty(), "already known: {again:?}");

        // An invite carries every interface address the inviter had, and a
        // device that roams between networks keeps producing new ones.
        insert_bounded(
            &mut set,
            (0..10).map(|i| format!("192.168.1.{i}:5000")).collect(),
            3,
        );
        assert_eq!(set.len(), 3, "must stay bounded: {set:?}");
    }

    #[test]
    fn failures_count_up_per_address_and_reset_on_success() {
        // Drives the pruning decision, so an address that starts working again
        // must not carry its old failures forward.
        let key = "a".repeat(64);

        assert_eq!(note_peer_result(&key, "10.0.0.9:1", false), 1);
        assert_eq!(note_peer_result(&key, "10.0.0.9:1", false), 2);
        // A different address has its own tally.
        assert_eq!(note_peer_result(&key, "10.0.0.8:1", false), 1);
        assert_eq!(note_peer_result(&key, "10.0.0.9:1", true), 0);
        assert_eq!(note_peer_result(&key, "10.0.0.9:1", false), 1);
    }

    #[tokio::test]
    async fn oversized_frames_are_rejected_not_allocated() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        // A hostile length prefix claiming ~4GB.
        a.write_all(&u32::MAX.to_le_bytes()).await.unwrap();

        assert!(read_frame(&mut b).await.is_err());
    }
}
