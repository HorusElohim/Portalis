//! Structural invariants every message must satisfy before dispatch.

use thiserror::Error;

use crate::limits::{CHALLENGE_BYTES, CONNECTION_ID_BYTES, MESSAGE_ID_BYTES};
use crate::v1;

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
    use prost::Message;

    use super::*;
    use crate::ids::{new_challenge, new_message_id};
    use crate::v1::envelope::Payload;
    use crate::v1::{Envelope, Ping, ProtocolRange, ServerHello};

    fn ping_envelope() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        }
    }

    fn valid_hello() -> ServerHello {
        ServerHello {
            connection_id: new_message_id(),
            challenge: new_challenge(),
            server_time_unix_ns: 1,
            supported_protocols: Some(ProtocolRange {
                minimum: 1,
                maximum: 1,
            }),
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
