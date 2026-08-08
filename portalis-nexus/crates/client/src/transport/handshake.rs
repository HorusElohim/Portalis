//! Establishing one authenticated-ready socket.

use futures_util::StreamExt;
use portalis_nexus_protocol::v1::{Envelope, ServerHello};
use portalis_nexus_protocol::{MAX_FRAME_BYTES, WEBSOCKET_SUBPROTOCOL, decode_frame};
use tokio::time::sleep;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use uuid::Uuid;

use crate::protocol::validate_hello;
use crate::reconnect::ReconnectPolicy;
use crate::transport::Socket;
use crate::transport::error::TransportError;

/// Connects once and validates the server's hello.
pub(crate) async fn handshake(endpoint: &str) -> Result<(Socket, ServerHello), TransportError> {
    let mut request = endpoint.into_client_request()?;
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(WEBSOCKET_SUBPROTOCOL),
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let (mut socket, response) = connect_async_with_config(request, Some(config), false).await?;
    let uses_expected_subprotocol = response
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .is_some_and(|value| value.as_bytes() == WEBSOCKET_SUBPROTOCOL.as_bytes());
    if !uses_expected_subprotocol {
        return Err(TransportError::MissingSubprotocol);
    }
    let hello = validate_hello(receive_envelope(&mut socket).await?)?;

    Ok((socket, hello))
}

/// Connects under a bounded exponential retry policy.
pub(crate) async fn handshake_with_retry(
    endpoint: &str,
    policy: &ReconnectPolicy,
) -> Result<(Socket, ServerHello), TransportError> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        match handshake(endpoint).await {
            Ok(connection) => return Ok(connection),
            Err(error) if !policy.can_retry_after(attempts) => {
                return Err(TransportError::ReconnectExhausted {
                    attempts,
                    source: Box::new(error),
                });
            }
            Err(_) => sleep(policy.delay_after_failure(attempts, random_entropy())).await,
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
