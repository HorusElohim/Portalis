//! Failures that can end a request or a connection.

use std::time::Duration;

use portalis_nexus_protocol::MAX_OUTBOUND_QUEUE;
use thiserror::Error;
use tokio_tungstenite::tungstenite::Error as WebSocketError;

use crate::error::ClientError;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    WebSocket(#[from] WebSocketError),
    #[error(transparent)]
    Frame(#[from] portalis_nexus_protocol::FrameError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("connection closed before a response arrived")]
    ConnectionClosed,
    #[error("expected a binary protobuf response")]
    UnexpectedWebSocketMessage,
    #[error("failed to connect after {attempts} attempts")]
    ReconnectExhausted {
        attempts: u32,
        #[source]
        source: Box<TransportError>,
    },
    #[error("the client has no live connection")]
    Disconnected,
    #[error("the outbound queue already holds {MAX_OUTBOUND_QUEUE} messages")]
    OutboundQueueFull,
    #[error("the handshake did not finish within {0:?}")]
    HandshakeTimeout(Duration),
    #[error("no response arrived within {0:?}")]
    RequestTimeout(Duration),
}
