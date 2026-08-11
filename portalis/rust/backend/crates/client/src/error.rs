//! Deterministic protocol failures, independent of any transport.

use portalis_nexus_protocol::v1::ProtocolErrorCode;
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
    #[error("a response was not correlated to its request")]
    InvalidCorrelation,
    #[error("pong nonce did not match the ping nonce")]
    InvalidPongNonce,
    #[error("the server returned an invalid {field}")]
    InvalidField { field: &'static str },
    #[error("at most {MAX_PENDING_REQUESTS} requests may be in flight at once")]
    TooManyPendingRequests,
    /// The server answered with a typed refusal rather than a result. Callers
    /// need the code to tell "pick another name" from "your device is
    /// revoked", so it is carried rather than flattened.
    #[error("the server refused the request: {message}")]
    Refused {
        code: ProtocolErrorCode,
        message: String,
    },
}
