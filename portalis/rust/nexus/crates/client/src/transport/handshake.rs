//! Establishing one authenticated-ready socket.

use std::time::Duration;

use futures_util::StreamExt;
use portalis_nexus_protocol::v1::{Envelope, ServerHello};
use portalis_nexus_protocol::{MAX_FRAME_BYTES, WEBSOCKET_SUBPROTOCOL, decode_frame};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tracing::debug;
use uuid::Uuid;

use crate::protocol::validate_hello;
use crate::reconnect::ReconnectPolicy;
use crate::transport::Socket;
use crate::transport::error::TransportError;

/// Connects once and validates the server's hello, within `limit`.
///
/// The bound matters: a peer that accepts the TCP connection but never finishes
/// the upgrade would otherwise stall the caller, or the supervisor, forever.
pub(crate) async fn handshake(
    endpoint: &str,
    limit: Duration,
) -> Result<(Socket, ServerHello), TransportError> {
    timeout(limit, connect_and_greet(endpoint))
        .await
        .map_err(|_| TransportError::HandshakeTimeout(limit))?
}

/// Requesting `Sec-WebSocket-Protocol` makes the handshake itself enforce
/// negotiation: tungstenite fails the connection when a server answers with a
/// different subprotocol or none at all, so no separate check is needed here.
async fn connect_and_greet(endpoint: &str) -> Result<(Socket, ServerHello), TransportError> {
    let mut request = endpoint.into_client_request()?;
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(WEBSOCKET_SUBPROTOCOL),
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let (mut socket, _response) = connect_async_with_config(request, Some(config), false).await?;
    let hello = validate_hello(receive_envelope(&mut socket).await?)?;

    Ok((socket, hello))
}

/// Connects under a bounded exponential retry policy.
pub(crate) async fn handshake_with_retry(
    endpoint: &str,
    policy: &ReconnectPolicy,
    limit: Duration,
) -> Result<(Socket, ServerHello), TransportError> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        match handshake(endpoint, limit).await {
            Ok(connection) => {
                debug!(attempts, "Nexus handshake succeeded");
                return Ok(connection);
            }
            Err(error) if !policy.can_retry_after(attempts) => {
                return Err(TransportError::ReconnectExhausted {
                    attempts,
                    source: Box::new(error),
                });
            }
            Err(error) => {
                let delay = policy.delay_after_failure(attempts, random_entropy());
                debug!(attempts, delay_ms = delay.as_millis(), %error, "Nexus handshake failed; retrying");
                sleep(delay).await;
            }
        }
    }
}

async fn receive_envelope(socket: &mut Socket) -> Result<Envelope, TransportError> {
    let Some(message) = socket.next().await else {
        return Err(TransportError::ConnectionClosed);
    };
    match message? {
        Message::Binary(frame) => Ok(decode_frame(&frame)?),
        Message::Close(_) => Err(TransportError::ConnectionClosed),
        _ => Err(TransportError::UnexpectedWebSocketMessage),
    }
}

/// Draws jitter entropy without adding a random-number dependency.
fn random_entropy() -> u64 {
    let bytes = *Uuid::new_v4().as_bytes();
    u64::from_le_bytes(
        bytes[..8]
            .try_into()
            .expect("a UUID always contains eight leading bytes"),
    )
}
