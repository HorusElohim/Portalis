//! Shared fixtures for the transport integration tests.
//!
//! Each test binary uses a subset of these helpers, so unused items are
//! expected here.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::{ClientConfig, DeviceSigner, ReconnectPolicy};
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{Envelope, Ping, ProtocolRange, ServerHello};
use portalis_nexus_protocol::{
    DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, SIGNATURE_BYTES, WEBSOCKET_SUBPROTOCOL, new_challenge,
    new_message_id,
};
use portalis_nexus_server::{AppState, binary_frame, hello_envelope, server_hello};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use x25519_dalek::{PublicKey, StaticSecret};

pub const SOCKET_ROUTE: &str = "/v1/socket";
/// Bounds every "eventually" assertion so a hung test fails instead of hanging.
pub const PATIENCE: Duration = Duration::from_secs(5);

/// A signer over a fixed key, so a test can re-create the same device.
///
/// Shared by every integration test file rather than defined locally in
/// each: a locally-defined type is its own distinct monomorphization target
/// for `ClientProtocol`'s generic methods, which fragments coverage of that
/// generic code across as many compiled copies as there are local
/// definitions. One shared type means one compiled copy to cover.
pub struct TestDevice {
    signing: SigningKey,
    /// A real X25519 secret, so a test can actually open what was sealed to
    /// this device rather than only assert that bytes moved.
    encryption: StaticSecret,
}

impl TestDevice {
    /// The private half, which only the device itself ever holds. Passed to
    /// [`portalis_nexus_protocol::open`] by whoever is standing in for this
    /// device; it never crosses the wire.
    pub fn encryption_secret_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        self.encryption.to_bytes()
    }
}

impl DeviceSigner for TestDevice {
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.signing.verifying_key().to_bytes()
    }

    fn encryption_public_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        PublicKey::from(&self.encryption).to_bytes()
    }

    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.signing.sign(payload).to_bytes()
    }
}

pub fn device(seed: u8) -> TestDevice {
    TestDevice {
        signing: SigningKey::from_bytes(&[seed; 32]),
        // A different byte pattern than the signing seed, so a test that
        // confuses the two keys fails rather than coincidentally passing.
        encryption: StaticSecret::from([seed.wrapping_add(128); 32]),
    }
}

/// Reserves an ephemeral port so a restarted server can rebind the address.
pub async fn reserve_address() -> SocketAddr {
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve test server address");
    let address = reservation.local_addr().expect("test server address");
    drop(reservation);
    address
}

pub async fn serve(address: SocketAddr, router: Router) -> JoinHandle<()> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind test server");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    })
}

/// Starts the real Nexus server, ready to serve.
///
/// The authority is bound to the address the test dials, because signatures
/// only verify when both sides name the same server.
pub async fn start_server(address: SocketAddr) -> (AppState, JoinHandle<()>) {
    let state = AppState::default().with_server_authority(&address.to_string());
    state.mark_ready();
    let handle = serve(address, portalis_nexus_server::app(&state)).await;
    (state, handle)
}

pub fn endpoint(address: SocketAddr) -> String {
    format!("ws://{address}{SOCKET_ROUTE}")
}

/// A reconnect policy that retries quickly enough for a test to observe it.
pub fn brisk_policy(maximum_attempts: u32) -> ReconnectPolicy {
    ReconnectPolicy::new(
        Duration::from_millis(20),
        Duration::from_millis(20),
        maximum_attempts,
    )
    .expect("valid test reconnect policy")
}

pub fn brisk_config(maximum_attempts: u32) -> ClientConfig {
    ClientConfig {
        reconnect: brisk_policy(maximum_attempts),
        ..ClientConfig::default()
    }
}

/// Polls until `condition` holds, failing the test if it never does.
pub async fn wait_until(label: &str, mut condition: impl FnMut() -> bool) {
    timeout(PATIENCE, async {
        while !condition() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting until {label}"));
}

/// Reads and discards until the peer leaves, so the closing handshake can
/// complete. These servers answer no protobuf request; they simply stay open.
async fn drain_inbound(mut socket: axum::extract::ws::WebSocket) {
    while socket.recv().await.is_some() {}
}

/// A valid greeting from the current protocol version.
fn greeting() -> Message {
    let state = AppState::default();
    binary_frame(&server_hello(state.protocol_policy(), 0))
}

/// A greeting advertising a protocol range this client cannot speak.
fn future_greeting() -> Message {
    binary_frame(&hello_envelope(
        ServerHello {
            connection_id: new_message_id(),
            challenge: new_challenge(),
            server_time_unix_ns: 0,
            supported_protocols: Some(ProtocolRange {
                minimum: 99,
                maximum: 99,
            }),
        },
        0,
    ))
}

pub fn unsolicited_ping(nonce: u64) -> Envelope {
    Envelope {
        message_id: new_message_id(),
        correlation_id: Vec::new(),
        timestamp_unix_ns: 1,
        payload: Some(Payload::Ping(Ping { nonce })),
    }
}

/// Greets correctly, then never answers another message.
pub fn silent_router() -> Router {
    Router::new().route(SOCKET_ROUTE, get(silent_upgrade))
}

async fn silent_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(|mut socket| async move {
            let _ = socket.send(greeting()).await;
            drain_inbound(socket).await;
        })
}

/// Upgrades without negotiating the protobuf subprotocol.
pub fn bare_router() -> Router {
    Router::new().route(SOCKET_ROUTE, get(bare_upgrade))
}

async fn bare_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket.on_upgrade(|mut socket| async move {
        let _ = socket.send(greeting()).await;
        drain_inbound(socket).await;
    })
}

/// Greets with a protocol range this client cannot speak.
pub fn future_router() -> Router {
    Router::new().route(SOCKET_ROUTE, get(future_upgrade))
}

async fn future_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(|mut socket| async move {
            let _ = socket.send(future_greeting()).await;
            drain_inbound(socket).await;
        })
}

/// Greets, pushes envelopes nothing asked for, then stays quiet.
pub fn event_router() -> Router {
    Router::new().route(SOCKET_ROUTE, get(event_upgrade))
}

async fn event_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(|mut socket| async move {
            let _ = socket.send(greeting()).await;
            for nonce in 1..=3 {
                let _ = socket.send(binary_frame(&unsolicited_ping(nonce))).await;
            }
            drain_inbound(socket).await;
        })
}

/// Greets, then answers every request with a pong, whatever was asked.
pub fn misanswering_router() -> Router {
    Router::new().route(SOCKET_ROUTE, get(misanswer_upgrade))
}

async fn misanswer_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(|mut socket| async move {
            let _ = socket.send(greeting()).await;
            while let Some(Ok(Message::Binary(frame))) = socket.recv().await {
                let Ok(request) = portalis_nexus_protocol::decode_frame(&frame) else {
                    return;
                };
                let pong = Envelope {
                    message_id: new_message_id(),
                    correlation_id: request.message_id,
                    timestamp_unix_ns: 1,
                    payload: Some(Payload::Pong(portalis_nexus_protocol::v1::Pong {
                        nonce: 0,
                    })),
                };
                let _ = socket.send(binary_frame(&pong)).await;
            }
        })
}

/// Greets, then closes its socket when the returned switch is flipped.
pub fn closable_router() -> (Router, Arc<watch::Sender<bool>>) {
    let close = Arc::new(watch::Sender::new(false));
    let handler = Arc::clone(&close);
    let router = Router::new().route(
        SOCKET_ROUTE,
        get(move |websocket: WebSocketUpgrade| {
            let close = Arc::clone(&handler);
            async move {
                websocket.protocols([WEBSOCKET_SUBPROTOCOL]).on_upgrade(
                    move |mut socket| async move {
                        let mut closing = close.subscribe();
                        let _ = socket.send(greeting()).await;
                        let _ = closing.changed().await;
                        let _ = socket.send(Message::Close(None)).await;
                    },
                )
            }
        }),
    );
    (router, close)
}
