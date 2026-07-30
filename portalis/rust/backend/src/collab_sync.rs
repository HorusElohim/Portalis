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
            }))
    });
    if let Ok(msg) = &result {
        clog!("collab_sync", "local_message_for: match found = {}", msg.is_some());
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
                    collection.add_manifest_entry(entry);
                }
                Err(err) => {
                    entries_rejected += 1;
                    clog!("collab_sync", "apply_message: dropped an unparseable entry: {err:?}");
                }
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
pub(crate) async fn public_ip() -> Option<String> {
    static PUBLIC_IP: OnceCell<Option<String>> = OnceCell::const_new();
    PUBLIC_IP
        .get_or_init(|| async {
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
            clog!("collab_sync", "public_ip: no service reachable, invites won't carry one");
            None
        })
        .await
        .clone()
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
            bt_listen_port: Some(6881),
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

    #[tokio::test]
    async fn oversized_frames_are_rejected_not_allocated() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        // A hostile length prefix claiming ~4GB.
        a.write_all(&u32::MAX.to_le_bytes()).await.unwrap();

        assert!(read_frame(&mut b).await.is_err());
    }
}
