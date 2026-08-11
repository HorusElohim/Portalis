//! Bounded encoding and decoding of one WebSocket binary message.

use prost::Message;
use thiserror::Error;

use crate::limits::MAX_FRAME_BYTES;
use crate::v1;
use crate::validate::ValidationError;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame exceeds the {MAX_FRAME_BYTES}-byte limit: {actual} bytes")]
    TooLarge { actual: usize },
    #[error("frame is not a valid protobuf envelope")]
    Malformed,
    #[error(transparent)]
    InvalidEnvelope(#[from] ValidationError),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::new_message_id;
    use crate::v1::envelope::Payload;
    use crate::v1::{Envelope, Ping, ProtocolError, ProtocolErrorCode};

    fn ping_envelope() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        }
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
            timestamp_unix_ns: 1,
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
}
