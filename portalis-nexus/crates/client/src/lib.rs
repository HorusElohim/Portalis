use std::time::Duration;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconnectPolicy {
    initial_delay: Duration,
    maximum_delay: Duration,
    maximum_attempts: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReconnectPolicyError {
    #[error("initial reconnect delay must be at least one millisecond")]
    ZeroInitialDelay,
    #[error("maximum reconnect delay must be at least the initial delay")]
    MaximumBeforeInitial,
    #[error("maximum connection attempts must be at least one")]
    ZeroMaximumAttempts,
}

impl ReconnectPolicy {
    /// Creates a bounded exponential reconnect policy.
    ///
    /// # Errors
    ///
    /// Returns [`ReconnectPolicyError`] when the delay bounds or attempt limit
    /// are invalid.
    pub fn new(
        initial_delay: Duration,
        maximum_delay: Duration,
        maximum_attempts: u32,
    ) -> Result<Self, ReconnectPolicyError> {
        if initial_delay.as_millis() == 0 {
            return Err(ReconnectPolicyError::ZeroInitialDelay);
        }
        if maximum_delay < initial_delay {
            return Err(ReconnectPolicyError::MaximumBeforeInitial);
        }
        if maximum_attempts == 0 {
            return Err(ReconnectPolicyError::ZeroMaximumAttempts);
        }
        Ok(Self {
            initial_delay,
            maximum_delay,
            maximum_attempts,
        })
    }

    #[must_use]
    pub fn can_retry_after(&self, attempts: u32) -> bool {
        attempts < self.maximum_attempts
    }

    /// Returns the delay to wait after `failures` consecutive failures.
    ///
    /// The initial delay doubles per failure, `entropy` spreads the result
    /// across 80%-120% of that value, and the maximum delay caps the jittered
    /// result so a policy never sleeps longer than its configured bound.
    #[must_use]
    pub fn delay_after_failure(&self, failures: u32, entropy: u64) -> Duration {
        let exponent = failures.saturating_sub(1).min(63);
        let base_millis = self
            .initial_delay
            .as_millis()
            .saturating_mul(1_u128 << exponent);
        let jitter_percent = 80_u128 + u128::from(entropy % 41);
        let jittered_millis = base_millis.saturating_mul(jitter_percent) / 100;
        let capped_millis = jittered_millis.min(self.maximum_delay.as_millis());
        Duration::from_millis(u64::try_from(capped_millis).unwrap_or(u64::MAX))
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::new(Duration::from_millis(100), Duration::from_secs(5), 10)
            .expect("the default reconnect policy is valid")
    }
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

    #[test]
    fn reconnect_policy_validates_its_bounds() {
        assert_eq!(
            ReconnectPolicy::new(Duration::ZERO, Duration::from_millis(1), 1),
            Err(ReconnectPolicyError::ZeroInitialDelay)
        );
        assert_eq!(
            ReconnectPolicy::new(Duration::from_millis(2), Duration::from_millis(1), 1),
            Err(ReconnectPolicyError::MaximumBeforeInitial)
        );
        assert_eq!(
            ReconnectPolicy::new(Duration::from_millis(1), Duration::from_millis(1), 0),
            Err(ReconnectPolicyError::ZeroMaximumAttempts)
        );
    }

    #[test]
    fn reconnect_policy_caps_exponential_jitter_and_attempts() {
        let policy =
            ReconnectPolicy::new(Duration::from_millis(100), Duration::from_millis(500), 3)
                .expect("valid reconnect policy");

        assert_eq!(policy.delay_after_failure(1, 0), Duration::from_millis(80));
        assert_eq!(
            policy.delay_after_failure(2, 40),
            Duration::from_millis(240)
        );
        assert_eq!(policy.delay_after_failure(4, 0), Duration::from_millis(500));
        assert!(policy.can_retry_after(2));
        assert!(!policy.can_retry_after(3));
        assert!(ReconnectPolicy::default().can_retry_after(9));
    }
}
