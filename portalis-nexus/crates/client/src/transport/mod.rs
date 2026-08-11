//! The supervised socket that carries commands and events.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use portalis_nexus_protocol::v1::{
    AddressFamily, Authenticated, DeviceLinked, Envelope, Friend, FriendAction, KeyEnvelopePut,
    LookupPeersResponse, PeerAnnounced, PeerWithdrawn, PublishShare, ResolveHandleResponse,
    ServerHello, ShareAccessGranted, ShareAccessRevoked, ShareSnapshot,
};
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, SessionBinding, encode_frame};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::config::ClientConfig;
use crate::protocol::{
    KeyEnvelopePage, validate_authenticated, validate_device_linked, validate_friend_event,
    validate_friend_list, validate_key_envelope_put, validate_key_envelopes,
    validate_peer_announced, validate_peer_lookup, validate_peer_withdrawn, validate_pong,
    validate_resolved, validate_share_access_granted, validate_share_access_revoked,
    validate_share_fetch, validate_share_handoff, validate_share_list, validate_share_published,
};
use crate::signer::DeviceSigner;
use crate::transport::connection::{Shared, start_connection, supervise};
use crate::transport::handshake::{handshake, handshake_with_retry};

mod connection;
mod error;
mod handshake;

pub use error::TransportError;

use portalis_nexus_protocol::MAX_OUTBOUND_QUEUE;

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A supervised connection to a Portalis Nexus endpoint.
///
/// The handle owns no socket. One supervisor task keeps a connection live,
/// reconnecting under the configured [`crate::ReconnectPolicy`] whenever the
/// socket ends, so commands issued through the handle survive a server restart.
pub struct NexusClient {
    shared: Arc<Shared>,
    events: Mutex<Option<mpsc::Receiver<Envelope>>>,
    supervisor: Option<JoinHandle<()>>,
}

impl NexusClient {
    /// Connects with a single handshake attempt, then supervises the socket.
    ///
    /// Use this when a caller wants an unreachable or misconfigured endpoint to
    /// fail immediately rather than after a full retry budget.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the WebSocket handshake, subprotocol, or
    /// protobuf hello is invalid.
    pub async fn connect(endpoint: &str) -> Result<Self, TransportError> {
        let config = ClientConfig::default();
        let connection = handshake(endpoint, config.request_timeout).await?;
        Ok(Self::supervised(endpoint, connection, config))
    }

    /// Connects under the configured retry policy, then supervises the socket.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ReconnectExhausted`] after the policy's final
    /// failed attempt, preserving the final transport error as its source.
    pub async fn connect_with_config(
        endpoint: &str,
        config: &ClientConfig,
    ) -> Result<Self, TransportError> {
        let connection =
            handshake_with_retry(endpoint, &config.reconnect, config.request_timeout).await?;
        Ok(Self::supervised(endpoint, connection, config.clone()))
    }

    /// Publishes the first connection before returning, so a caller can send a
    /// command immediately without racing the supervisor's first iteration.
    fn supervised(endpoint: &str, connection: (Socket, ServerHello), config: ClientConfig) -> Self {
        let (events, inbox) = mpsc::channel(MAX_OUTBOUND_QUEUE);
        let shared = Arc::new(Shared::new(
            events,
            config.request_timeout,
            authority_of(endpoint),
        ));
        // Subscribed here, not inside the task: a caller may shut down before
        // the supervisor has run for the first time.
        let shutdown = shared.shutdown.subscribe();
        let first = start_connection(&shared, connection);
        let supervisor = tokio::spawn(supervise(
            Arc::clone(&shared),
            endpoint.to_owned(),
            config.reconnect,
            first,
            shutdown,
        ));

        Self {
            shared,
            events: Mutex::new(Some(inbox)),
            supervisor: Some(supervisor),
        }
    }

    /// Returns the hello of the current connection, if one is live.
    #[must_use]
    pub fn hello(&self) -> Option<ServerHello> {
        self.shared.hello()
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.shared.outbound().is_some()
    }

    /// Returns how many requests are awaiting a response right now.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.shared.pending.len()
    }

    /// Takes the stream of server-initiated envelopes, which is available once.
    ///
    /// # Panics
    ///
    /// Panics only if a previous caller panicked while taking the stream.
    #[must_use]
    pub fn events(&self) -> Option<mpsc::Receiver<Envelope>> {
        self.events
            .lock()
            .expect("the event stream slot is never poisoned")
            .take()
    }

    /// Sends one envelope and waits for the response that correlates with it.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the client is disconnected, its queues
    /// are saturated, the connection ends first, or no response arrives within
    /// the configured request timeout.
    pub async fn request(&self, request: &Envelope) -> Result<Envelope, TransportError> {
        let frame = encode_frame(request)?;
        let response = self.shared.pending.register(request.message_id.clone())?;

        // Scoped so no queue sender is held across the await below, which would
        // stop the writer task from observing a closed queue.
        let queued = {
            let outbound = self.shared.outbound().ok_or(TransportError::Disconnected)?;
            outbound.try_send(Message::Binary(frame.into()))
        };
        if let Err(error) = queued {
            self.shared.pending.cancel(&request.message_id);
            return Err(match error {
                mpsc::error::TrySendError::Full(_) => TransportError::OutboundQueueFull,
                mpsc::error::TrySendError::Closed(_) => TransportError::Disconnected,
            });
        }

        match timeout(self.shared.request_timeout, response).await {
            Ok(Ok(envelope)) => Ok(envelope),
            Ok(Err(_)) => Err(TransportError::ConnectionClosed),
            Err(_) => {
                self.shared.pending.cancel(&request.message_id);
                Err(TransportError::RequestTimeout(self.shared.request_timeout))
            }
        }
    }

    /// Claims `username` and enrols the signing device as its first.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the client is disconnected or the
    /// server refuses the request.
    pub async fn register<S: DeviceSigner + ?Sized>(
        &self,
        username: &str,
        signer: &S,
    ) -> Result<Authenticated, TransportError> {
        let hello = self.hello().ok_or(TransportError::Disconnected)?;
        let request = self.shared.protocol.register(
            &binding(&hello, self.authority()),
            username,
            signer,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_authenticated(&request, &response)?)
    }

    /// Proves this device is enrolled, binding the connection to its identity.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the client is disconnected or the
    /// server refuses the request.
    pub async fn authenticate<S: DeviceSigner + ?Sized>(
        &self,
        signer: &S,
    ) -> Result<Authenticated, TransportError> {
        let hello = self.hello().ok_or(TransportError::Disconnected)?;
        let request = self.shared.protocol.authenticate(
            &binding(&hello, self.authority()),
            signer,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_authenticated(&request, &response)?)
    }

    /// Approves a new device's keys, signed by this already-enrolled one.
    ///
    /// Needs no [`crate::ReconnectPolicy`]-tracked handshake state beyond the
    /// authority to sign against: the approval itself is not bound to this
    /// connection, so it remains valid however the candidate device submits
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the client is disconnected or the
    /// server refuses the request.
    pub async fn link_device<S: DeviceSigner + ?Sized>(
        &self,
        candidate_signing_public_key: &[u8],
        candidate_encryption_public_key: &[u8],
        approver: &S,
    ) -> Result<DeviceLinked, TransportError> {
        let request = self.shared.protocol.link_device(
            self.authority(),
            candidate_signing_public_key,
            candidate_encryption_public_key,
            approver,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_device_linked(&request, &response)?)
    }

    /// Stores a sealed share key for one of this user's own devices.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the server refuses or cannot answer.
    pub async fn put_key_envelope(
        &self,
        share_id: &[u8],
        recipient_device_id: &[u8],
        ephemeral_public_key: &[u8],
        ciphertext: &[u8],
    ) -> Result<KeyEnvelopePut, TransportError> {
        let request = self.shared.protocol.put_key_envelope(
            share_id,
            recipient_device_id,
            ephemeral_public_key,
            ciphertext,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_key_envelope_put(&request, &response)?)
    }

    /// One bounded page of envelopes addressed to this connection's own device.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the server refuses or cannot answer.
    pub async fn list_key_envelopes(
        &self,
        after_share_id: Option<&[u8]>,
    ) -> Result<KeyEnvelopePage, TransportError> {
        let request = self
            .shared
            .protocol
            .list_key_envelopes(after_share_id, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_key_envelopes(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when publication is refused or cannot be delivered.
    pub async fn publish_share(
        &self,
        share_id: &[u8],
        revision: u64,
        prior_snapshot_id: Option<&[u8]>,
        snapshot_id: &[u8],
        capsule: &[u8],
        capsule_signature: &[u8],
    ) -> Result<ShareSnapshot, TransportError> {
        let request = self.shared.protocol.publish_share(
            PublishShare {
                share_id: share_id.to_vec(),
                revision,
                prior_snapshot_id: prior_snapshot_id.unwrap_or_default().to_vec(),
                snapshot_id: snapshot_id.to_vec(),
                capsule: capsule.to_vec(),
                capsule_signature: capsule_signature.to_vec(),
            },
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_share_published(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the list cannot be fetched or validated.
    pub async fn list_shares(&self) -> Result<Vec<ShareSnapshot>, TransportError> {
        let request = self.shared.protocol.list_shares(now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_share_list(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the share is private, absent, or unavailable.
    pub async fn fetch_share(&self, share_id: &[u8]) -> Result<ShareSnapshot, TransportError> {
        let request = self.shared.protocol.fetch_share(share_id, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_share_fetch(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the grant is unauthorized or unavailable.
    pub async fn grant_share_access(
        &self,
        share_id: &[u8],
        member_user_id: &[u8],
    ) -> Result<ShareAccessGranted, TransportError> {
        let request =
            self.shared
                .protocol
                .grant_share_access(share_id, member_user_id, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_share_access_granted(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the revocation is unauthorized or unavailable.
    pub async fn revoke_share_access(
        &self,
        share_id: &[u8],
        member_user_id: &[u8],
    ) -> Result<ShareAccessRevoked, TransportError> {
        let request =
            self.shared
                .protocol
                .revoke_share_access(share_id, member_user_id, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_share_access_revoked(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the handoff is unauthorized or unavailable.
    pub async fn share_handoff(
        &self,
        share_id: &[u8],
        recipient_device_id: &[u8],
        ciphertext: &[u8],
    ) -> Result<(), TransportError> {
        let request = self.shared.protocol.share_handoff(
            share_id,
            recipient_device_id,
            ciphertext,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_share_handoff(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the lease is invalid or cannot be announced.
    pub async fn announce_peer(
        &self,
        info_hash: &[u8],
        listen_port: u16,
        address_family: AddressFamily,
        transport_capabilities: u32,
        requested_lease_seconds: u32,
    ) -> Result<PeerAnnounced, TransportError> {
        let request = self.shared.protocol.announce_peer(
            info_hash,
            listen_port,
            address_family,
            transport_capabilities,
            requested_lease_seconds,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_peer_announced(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when discovery cannot be queried or validated.
    pub async fn lookup_peers(
        &self,
        info_hash: &[u8],
        address_family: AddressFamily,
        transport_capabilities: u32,
    ) -> Result<LookupPeersResponse, TransportError> {
        let request = self.shared.protocol.lookup_peers(
            info_hash,
            address_family,
            transport_capabilities,
            now_unix_ns(),
        );
        let response = self.request(&request).await?;
        Ok(validate_peer_lookup(&request, &response)?)
    }

    /// # Errors
    /// Returns [`TransportError`] when the lease cannot be withdrawn.
    pub async fn withdraw_peer(&self, info_hash: &[u8]) -> Result<PeerWithdrawn, TransportError> {
        let request = self.shared.protocol.withdraw_peer(info_hash, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_peer_withdrawn(&request, &response)?)
    }

    /// The authority signatures are bound to, taken from the endpoint dialled.
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.shared.server_authority
    }

    /// Finds the user behind a handle.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the server refuses or cannot answer.
    pub async fn resolve_handle(
        &self,
        handle: &str,
    ) -> Result<ResolveHandleResponse, TransportError> {
        let request = self.shared.protocol.resolve_handle(handle, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_resolved(&request, &response)?)
    }

    /// Applies a friend action against `peer`.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the server refuses or cannot answer.
    pub async fn friend_command(
        &self,
        action: FriendAction,
        peer: &[u8],
    ) -> Result<Friend, TransportError> {
        let request = self
            .shared
            .protocol
            .friend_command(action, peer, now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_friend_event(&request, &response)?)
    }

    /// Lists every friendship this user is part of.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the server refuses or cannot answer.
    pub async fn list_friends(&self) -> Result<Vec<Friend>, TransportError> {
        let request = self.shared.protocol.list_friends(now_unix_ns());
        let response = self.request(&request).await?;
        Ok(validate_friend_list(&request, &response)?)
    }

    /// Sends a ping and verifies the correlated pong response.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when sending or validating the response fails.
    pub async fn ping(&self, nonce: u64) -> Result<Envelope, TransportError> {
        let request = self.shared.protocol.ping(nonce, now_unix_ns());
        let response = self.request(&request).await?;
        validate_pong(&request, &response)?;
        Ok(response)
    }

    /// Stops supervising, closes the live socket, and waits for it to finish.
    pub async fn shutdown(mut self) {
        self.shared.shutdown.send_replace(true);
        if let Some(supervisor) = self.supervisor.take() {
            let _ = supervisor.await;
        }
    }
}

/// Reports connection state only. A derived implementation would print the
/// server challenge, which `SPEC.md` section 15 keeps out of logs.
impl std::fmt::Debug for NexusClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NexusClient")
            .field("connected", &self.is_connected())
            .field("in_flight", &self.in_flight())
            .finish()
    }
}

impl Drop for NexusClient {
    fn drop(&mut self) {
        self.shared.shutdown.send_replace(true);
        if let Some(supervisor) = &self.supervisor {
            supervisor.abort();
        }
    }
}

fn now_unix_ns() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos(),
    )
    .expect("nanoseconds since the Unix epoch fit in u64 until 2554")
}

/// Builds the session binding a signature is scoped to, from the hello the
/// server sent and the authority this client dialled.
fn binding<'a>(hello: &'a ServerHello, server_authority: &'a str) -> SessionBinding<'a> {
    SessionBinding {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        server_authority,
        connection_id: &hello.connection_id,
        challenge: &hello.challenge,
        server_time_unix_ns: hello.server_time_unix_ns,
    }
}

/// The `host:port` a WebSocket endpoint addresses.
///
/// Signatures are bound to it, so what the client signs is the server it meant
/// to reach rather than whatever a relay claims to be.
#[must_use]
pub fn authority_of(endpoint: &str) -> String {
    endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}
