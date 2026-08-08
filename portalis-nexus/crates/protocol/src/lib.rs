use thiserror::Error;
use uuid::Uuid;

#[allow(clippy::doc_markdown, clippy::must_use_candidate)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/portalis.protocol.v1.rs"));
}

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const MESSAGE_ID_BYTES: usize = 16;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

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

#[must_use]
pub fn new_message_id() -> Vec<u8> {
    Uuid::now_v7().as_bytes().to_vec()
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
    use prost::Message;

    use super::v1::envelope::Payload;
    use super::v1::{Envelope, Ping};
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
}
