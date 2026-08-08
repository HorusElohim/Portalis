use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{Envelope, Ping, Pong, ServerHello};
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, new_message_id, validate_server_hello};
use thiserror::Error;

mod transport;

pub use transport::{NexusClient, TransportError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientProtocol {
    version: u32,
}

impl Default for ClientProtocol {
    fn default() -> Self {
        Self {
            version: CURRENT_PROTOCOL_VERSION,
        }
    }
}

impl ClientProtocol {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub fn ping(&self, nonce: u64, sent_at_unix_ms: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms,
            payload: Some(Payload::Ping(Ping { nonce })),
        }
    }
}

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
}

pub(crate) fn validate_hello(envelope: Envelope) -> Result<ServerHello, ClientError> {
    let Some(Payload::ServerHello(hello)) = envelope.payload else {
        return Err(ClientError::UnexpectedEnvelope {
            expected: "ServerHello",
        });
    };
    validate_server_hello(&hello)?;
    let protocols = hello
        .supported_protocols
        .as_ref()
        .expect("a validated server hello has a protocol range");
    if !(protocols.minimum..=protocols.maximum).contains(&CURRENT_PROTOCOL_VERSION) {
        return Err(ClientError::UnsupportedProtocolVersion);
    }
    Ok(hello)
}

pub(crate) fn validate_pong(request: &Envelope, response: &Envelope) -> Result<(), ClientError> {
    let Some(Payload::Pong(Pong { nonce })) = &response.payload else {
        return Err(ClientError::UnexpectedEnvelope { expected: "Pong" });
    };
    if response.correlation_id != request.message_id {
        return Err(ClientError::InvalidPongCorrelation);
    }
    let Some(Payload::Ping(Ping {
        nonce: request_nonce,
    })) = &request.payload
    else {
        return Err(ClientError::UnexpectedEnvelope { expected: "Ping" });
    };
    if nonce != request_nonce {
        return Err(ClientError::InvalidPongNonce);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use portalis_nexus_protocol::v1::{ProtocolRange, ServerHello};
    use portalis_nexus_protocol::{new_challenge, new_message_id};

    fn hello_envelope(range: ProtocolRange) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::ServerHello(ServerHello {
                connection_id: new_message_id(),
                challenge: new_challenge(),
                server_time_unix_ms: 1,
                supported_protocols: Some(range),
            })),
        }
    }

    fn ping_envelope(nonce: u64) -> Envelope {
        ClientProtocol::default().ping(nonce, 1)
    }

    fn pong_envelope(request: &Envelope, nonce: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: request.message_id.clone(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Pong(Pong { nonce })),
        }
    }

    #[test]
    fn builds_valid_ping_for_current_protocol() {
        let client = ClientProtocol::default();
        let envelope = client.ping(42, 1000);

        assert_eq!(client.version(), CURRENT_PROTOCOL_VERSION);
        assert_eq!(envelope.sent_at_unix_ms, 1000);
        assert_eq!(envelope.validate(), Ok(()));
        assert_eq!(envelope.payload, Some(Payload::Ping(Ping { nonce: 42 })));
    }

    #[test]
    fn accepts_compatible_server_hello() {
        let hello = validate_hello(hello_envelope(ProtocolRange {
            minimum: CURRENT_PROTOCOL_VERSION,
            maximum: CURRENT_PROTOCOL_VERSION,
        }))
        .expect("compatible hello");

        assert_eq!(hello.supported_protocols.expect("range").minimum, 1);
    }

    #[test]
    fn rejects_unexpected_invalid_or_unsupported_hello() {
        assert_eq!(
            validate_hello(ping_envelope(7)),
            Err(ClientError::UnexpectedEnvelope {
                expected: "ServerHello"
            })
        );
        let invalid = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::ServerHello(ServerHello {
                connection_id: vec![0; 15],
                challenge: new_challenge(),
                server_time_unix_ms: 1,
                supported_protocols: Some(ProtocolRange {
                    minimum: CURRENT_PROTOCOL_VERSION,
                    maximum: CURRENT_PROTOCOL_VERSION,
                }),
            })),
        };
        assert_eq!(
            validate_hello(invalid),
            Err(ClientError::ServerHello(
                portalis_nexus_protocol::ServerHelloValidationError::InvalidConnectionId {
                    actual: 15,
                }
            ))
        );
        assert_eq!(
            validate_hello(hello_envelope(ProtocolRange {
                minimum: CURRENT_PROTOCOL_VERSION + 1,
                maximum: CURRENT_PROTOCOL_VERSION + 1,
            })),
            Err(ClientError::UnsupportedProtocolVersion)
        );
    }

    #[test]
    fn validates_correlated_pongs() {
        let request = ping_envelope(42);
        let response = pong_envelope(&request, 42);

        assert_eq!(validate_pong(&request, &response), Ok(()));

        let mut invalid_correlation = response.clone();
        invalid_correlation.correlation_id = new_message_id();
        assert_eq!(
            validate_pong(&request, &invalid_correlation),
            Err(ClientError::InvalidPongCorrelation)
        );
        assert_eq!(
            validate_pong(&request, &pong_envelope(&request, 7)),
            Err(ClientError::InvalidPongNonce)
        );
        assert_eq!(
            validate_pong(&request, &ping_envelope(42)),
            Err(ClientError::UnexpectedEnvelope { expected: "Pong" })
        );
        let non_ping_request = pong_envelope(&request, 42);
        assert_eq!(
            validate_pong(&non_ping_request, &pong_envelope(&non_ping_request, 42)),
            Err(ClientError::UnexpectedEnvelope { expected: "Ping" })
        );
    }
}
