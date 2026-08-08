use prost::Message;
use thiserror::Error;
use uuid::Uuid;

#[allow(clippy::doc_markdown, clippy::must_use_candidate)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/portalis.protocol.v1.rs"));
}

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const MESSAGE_ID_BYTES: usize = 16;
pub const CONNECTION_ID_BYTES: usize = 16;
pub const CHALLENGE_BYTES: usize = 32;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const WEBSOCKET_SUBPROTOCOL: &str = "portalis.protobuf.v1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("message_id must contain exactly {MESSAGE_ID_BYTES} bytes, got {actual}")]
    InvalidMessageId { actual: usize },
    #[error(
        "correlation_id must be empty or contain exactly {MESSAGE_ID_BYTES} bytes, got {actual}"
    )]
    InvalidCorrelationId { actual: usize },
    #[error("envelope payload is required")]
    MissingPayload,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServerHelloValidationError {
    #[error("connection_id must contain exactly {CONNECTION_ID_BYTES} bytes, got {actual}")]
    InvalidConnectionId { actual: usize },
    #[error("challenge must contain exactly {CHALLENGE_BYTES} bytes, got {actual}")]
    InvalidChallenge { actual: usize },
    #[error("supported_protocols is required")]
    MissingProtocolRange,
    #[error("supported protocol range minimum {minimum} exceeds maximum {maximum}")]
    InvalidProtocolRange { minimum: u32, maximum: u32 },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame exceeds the {MAX_FRAME_BYTES}-byte limit: {actual} bytes")]
    TooLarge { actual: usize },
    #[error("frame is not a valid protobuf envelope")]
    Malformed,
    #[error(transparent)]
    InvalidEnvelope(#[from] ValidationError),
}

#[must_use]
pub fn new_message_id() -> Vec<u8> {
    Uuid::now_v7().as_bytes().to_vec()
}

#[must_use]
pub fn new_challenge() -> Vec<u8> {
    let mut challenge = Vec::with_capacity(CHALLENGE_BYTES);
    challenge.extend_from_slice(Uuid::new_v4().as_bytes());
    challenge.extend_from_slice(Uuid::new_v4().as_bytes());
    challenge
}

/// Encodes one bounded protobuf WebSocket binary message.
///
/// # Errors
///
/// Returns [`FrameError`] when the envelope is invalid or encoded data exceeds
/// the protocol frame limit.
pub fn encode_frame(envelope: &v1::Envelope) -> Result<Vec<u8>, FrameError> {
    envelope.validate()?;
    let bytes = envelope.encode_to_vec();
    validate_frame_size(bytes.len())?;
    Ok(bytes)
}

/// Decodes and validates one bounded protobuf WebSocket binary message.
///
/// # Errors
///
/// Returns [`FrameError`] when the frame is too large, malformed, or violates
/// the envelope invariants.
pub fn decode_frame(bytes: &[u8]) -> Result<v1::Envelope, FrameError> {
    validate_frame_size(bytes.len())?;
    let envelope = v1::Envelope::decode(bytes).map_err(|_| FrameError::Malformed)?;
    envelope.validate()?;
    Ok(envelope)
}

/// Validates a WebSocket binary payload length before allocating or decoding.
///
/// # Errors
///
/// Returns [`FrameError::TooLarge`] when the payload exceeds the protocol
/// limit.
pub fn validate_frame_size(actual: usize) -> Result<(), FrameError> {
    if actual > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge { actual });
    }
    Ok(())
}

/// Verifies the fields that make a server hello safe to use for authentication.
///
/// # Errors
///
/// Returns [`ServerHelloValidationError`] when a required field is absent or
/// has an invalid fixed length or protocol range.
pub fn validate_server_hello(hello: &v1::ServerHello) -> Result<(), ServerHelloValidationError> {
    if hello.connection_id.len() != CONNECTION_ID_BYTES {
        return Err(ServerHelloValidationError::InvalidConnectionId {
            actual: hello.connection_id.len(),
        });
    }
    if hello.challenge.len() != CHALLENGE_BYTES {
        return Err(ServerHelloValidationError::InvalidChallenge {
            actual: hello.challenge.len(),
        });
    }
    let Some(protocols) = &hello.supported_protocols else {
        return Err(ServerHelloValidationError::MissingProtocolRange);
    };
    if protocols.minimum > protocols.maximum {
        return Err(ServerHelloValidationError::InvalidProtocolRange {
            minimum: protocols.minimum,
            maximum: protocols.maximum,
        });
    }
    Ok(())
}

impl v1::Envelope {
    /// Verifies the structural invariants required before dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] when an identifier has the wrong length or
    /// the envelope has no payload.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.message_id.len() != MESSAGE_ID_BYTES {
            return Err(ValidationError::InvalidMessageId {
                actual: self.message_id.len(),
            });
        }
        if !self.correlation_id.is_empty() && self.correlation_id.len() != MESSAGE_ID_BYTES {
            return Err(ValidationError::InvalidCorrelationId {
                actual: self.correlation_id.len(),
            });
        }
        if self.payload.is_none() {
            return Err(ValidationError::MissingPayload);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::v1::envelope::Payload;
    use super::v1::{Envelope, Ping, ProtocolError, ProtocolErrorCode, ProtocolRange, ServerHello};
    use super::*;

    fn ping_envelope() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        }
    }

    #[test]
    fn valid_envelope_round_trips() {
        let envelope = ping_envelope();
        let bytes = envelope.encode_to_vec();
        let decoded = Envelope::decode(bytes.as_slice()).expect("valid protobuf");

        assert_eq!(decoded, envelope);
        assert_eq!(decoded.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_message_id() {
        let mut envelope = ping_envelope();
        envelope.message_id.pop();

        assert_eq!(
            envelope.validate(),
            Err(ValidationError::InvalidMessageId { actual: 15 })
        );
    }

    #[test]
    fn rejects_invalid_correlation_id() {
        let mut envelope = ping_envelope();
        envelope.correlation_id = vec![0; 17];

        assert_eq!(
            envelope.validate(),
            Err(ValidationError::InvalidCorrelationId { actual: 17 })
        );
    }

    #[test]
    fn accepts_complete_correlation_id() {
        let mut envelope = ping_envelope();
        envelope.correlation_id = new_message_id();

        assert_eq!(envelope.validate(), Ok(()));
    }

    #[test]
    fn rejects_missing_payload() {
        let mut envelope = ping_envelope();
        envelope.payload = None;

        assert_eq!(envelope.validate(), Err(ValidationError::MissingPayload));
    }

    #[test]
    fn rejects_oversized_frame_without_allocating() {
        assert!(matches!(
            validate_frame_size(MAX_FRAME_BYTES + 1),
            Err(FrameError::TooLarge { actual }) if actual == MAX_FRAME_BYTES + 1
        ));
        assert!(validate_frame_size(MAX_FRAME_BYTES).is_ok());
    }

    #[test]
    fn rejects_malformed_or_invalid_frames() {
        assert_eq!(decode_frame(&[0xff]), Err(FrameError::Malformed));
        assert_eq!(
            decode_frame(&[]),
            Err(FrameError::InvalidEnvelope(
                ValidationError::InvalidMessageId { actual: 0 }
            ))
        );
    }

    #[test]
    fn encodes_and_decodes_valid_frames() {
        let envelope = ping_envelope();

        assert_eq!(
            decode_frame(&encode_frame(&envelope).expect("valid frame")).expect("valid frame"),
            envelope
        );
    }

    #[test]
    fn rejects_invalid_or_oversized_encoded_frames() {
        let mut invalid = ping_envelope();
        invalid.message_id.clear();
        assert_eq!(
            encode_frame(&invalid),
            Err(FrameError::InvalidEnvelope(
                ValidationError::InvalidMessageId { actual: 0 }
            ))
        );

        let oversized = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::ProtocolError(ProtocolError {
                code: ProtocolErrorCode::InvalidMessage as i32,
                message: "x".repeat(MAX_FRAME_BYTES),
                retry_after_ms: None,
                retryable: false,
            })),
        };
        let encoded_length = oversized.encode_to_vec().len();
        assert_eq!(
            encode_frame(&oversized),
            Err(FrameError::TooLarge {
                actual: encoded_length,
            })
        );
        assert_eq!(
            decode_frame(&vec![0; MAX_FRAME_BYTES + 1]),
            Err(FrameError::TooLarge {
                actual: MAX_FRAME_BYTES + 1,
            })
        );
    }

    fn valid_hello() -> ServerHello {
        ServerHello {
            connection_id: new_message_id(),
            challenge: new_challenge(),
            server_time_unix_ms: 1,
            supported_protocols: Some(ProtocolRange {
                minimum: 1,
                maximum: 1,
            }),
        }
    }

    #[test]
    fn validates_server_hello() {
        let hello = valid_hello();

        assert_eq!(validate_server_hello(&hello), Ok(()));
        assert_eq!(hello.challenge.len(), CHALLENGE_BYTES);
    }

    #[test]
    fn rejects_invalid_server_hello_fields() {
        let mut hello = valid_hello();
        hello.connection_id.pop();
        assert_eq!(
            validate_server_hello(&hello),
            Err(ServerHelloValidationError::InvalidConnectionId { actual: 15 })
        );

        hello = valid_hello();
        hello.challenge.pop();
        assert_eq!(
            validate_server_hello(&hello),
            Err(ServerHelloValidationError::InvalidChallenge { actual: 31 })
        );

        hello = valid_hello();
        hello.supported_protocols = None;
        assert_eq!(
            validate_server_hello(&hello),
            Err(ServerHelloValidationError::MissingProtocolRange)
        );

        hello = valid_hello();
        hello.supported_protocols = Some(ProtocolRange {
            minimum: 2,
            maximum: 1,
        });
        assert_eq!(
            validate_server_hello(&hello),
            Err(ServerHelloValidationError::InvalidProtocolRange {
                minimum: 2,
                maximum: 1,
            })
        );
    }
}
