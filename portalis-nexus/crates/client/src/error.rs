//! Deterministic protocol failures, independent of any transport.

use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, MAX_PENDING_REQUESTS};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClientError {
    #[error(transparent)]
    ServerHello(#[from] portalis_nexus_protocol::ServerHelloValidationError),
    #[error("server does not support protocol version {CURRENT_PROTOCOL_VERSION}")]
    UnsupportedProtocolVersion,
    #[error("expected {expected} but received a different envelope payload")]
    UnexpectedEnvelope { expected: &'static str },
    #[error("pong correlation_id did not match the ping message_id")]
    InvalidPongCorrelation,
    #[error("pong nonce did not match the ping nonce")]
    InvalidPongNonce,
    #[error("at most {MAX_PENDING_REQUESTS} requests may be in flight at once")]
    TooManyPendingRequests,
}
