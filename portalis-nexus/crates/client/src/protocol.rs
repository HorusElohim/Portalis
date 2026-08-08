//! Deterministic client-side protocol rules, with no sockets involved.

use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    AuthenticateDevice, Authenticated, Envelope, Ping, Pong, ProtocolErrorCode, RegisterUser,
    ServerHello,
};
use portalis_nexus_protocol::{
    CURRENT_PROTOCOL_VERSION, SessionBinding, authentication_payload, new_message_id,
    registration_payload, validate_server_hello,
};

use crate::error::ClientError;
use crate::signer::DeviceSigner;

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

    /// Builds a signed request to claim `username`.
    #[must_use]
    pub fn register<S: DeviceSigner + ?Sized>(
        &self,
        binding: &SessionBinding<'_>,
        username: &str,
        signer: &S,
        sent_at_unix_ms: u64,
    ) -> Envelope {
        let public_key = signer.public_key();
        let payload = registration_payload(binding, username, &public_key);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms,
            payload: Some(Payload::RegisterUser(RegisterUser {
                requested_username: username.to_owned(),
                device_public_key: public_key.to_vec(),
                signature: signer.sign(&payload).to_vec(),
            })),
        }
    }

    /// Builds a signed request to prove this device is enrolled.
    #[must_use]
    pub fn authenticate<S: DeviceSigner + ?Sized>(
        &self,
        binding: &SessionBinding<'_>,
        signer: &S,
        sent_at_unix_ms: u64,
    ) -> Envelope {
        let public_key = signer.public_key();
        let payload = authentication_payload(binding, &public_key);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms,
            payload: Some(Payload::AuthenticateDevice(AuthenticateDevice {
                device_public_key: public_key.to_vec(),
                signature: signer.sign(&payload).to_vec(),
            })),
        }
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

/// Accepts a server hello only when it is well formed and version-compatible.
///
/// # Errors
///
/// Returns [`ClientError`] when the envelope is not a hello, the hello is
/// invalid, or its protocol range excludes this client.
///
/// # Panics
///
/// Panics if validation accepted a hello without a protocol range, which the
/// validator forbids.
pub fn validate_hello(envelope: Envelope) -> Result<ServerHello, ClientError> {
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

/// Verifies that a pong answers the ping that requested it.
///
/// # Errors
///
/// Returns [`ClientError`] when the payload, correlation ID, or nonce does not
/// match the request.
pub fn validate_pong(request: &Envelope, response: &Envelope) -> Result<(), ClientError> {
    let Some(Payload::Pong(Pong { nonce })) = &response.payload else {
        return Err(ClientError::UnexpectedEnvelope { expected: "Pong" });
    };
    if response.correlation_id != request.message_id {
        return Err(ClientError::InvalidCorrelation);
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

/// Reads the identity a server confirmed.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not an `Authenticated`, is not
/// correlated to the request, or names a protocol version this client does not
/// speak.
pub fn validate_authenticated(
    request: &Envelope,
    response: &Envelope,
) -> Result<Authenticated, ClientError> {
    if response.correlation_id != request.message_id {
        return Err(ClientError::InvalidCorrelation);
    }
    match &response.payload {
        Some(Payload::Authenticated(identity)) => {
            if identity.protocol_version != CURRENT_PROTOCOL_VERSION {
                return Err(ClientError::UnsupportedProtocolVersion);
            }
            Ok(identity.clone())
        }
        Some(Payload::ProtocolError(refusal)) => Err(ClientError::Refused {
            code: ProtocolErrorCode::try_from(refusal.code)
                .unwrap_or(ProtocolErrorCode::Unspecified),
            message: refusal.message.clone(),
        }),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "Authenticated",
        }),
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::new_challenge;
    use portalis_nexus_protocol::v1::ProtocolRange;

    use super::*;

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
            Err(ClientError::InvalidCorrelation)
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

    fn authenticated_envelope(request: &Envelope, protocol_version: u32) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: request.message_id.clone(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Authenticated(Authenticated {
                user_id: vec![1; 16],
                device_id: vec![2; 32],
                username: "Ada".to_owned(),
                discriminator: "7Q2XZ".to_owned(),
                protocol_version,
            })),
        }
    }

    fn claim_of(envelope: &Envelope) -> Option<RegisterUser> {
        match &envelope.payload {
            Some(Payload::RegisterUser(claim)) => Some(claim.clone()),
            _ => None,
        }
    }

    fn proof_of(envelope: &Envelope) -> Option<AuthenticateDevice> {
        match &envelope.payload {
            Some(Payload::AuthenticateDevice(proof)) => Some(proof.clone()),
            _ => None,
        }
    }

    struct FixedSigner;

    impl DeviceSigner for FixedSigner {
        fn public_key(&self) -> [u8; 32] {
            [7; 32]
        }

        fn sign(&self, payload: &[u8]) -> [u8; 64] {
            let mut signature = [0_u8; 64];
            signature[0] = u8::try_from(payload.len() % 251).unwrap_or_default();
            signature
        }
    }

    fn binding<'a>(challenge: &'a [u8], connection_id: &'a [u8]) -> SessionBinding<'a> {
        SessionBinding {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            server_authority: "nexus.portalis.test",
            connection_id,
            challenge,
            server_time_unix_ms: 1,
        }
    }

    #[test]
    fn builds_signed_identity_commands() {
        let client = ClientProtocol::default();
        let session = binding(&[1; 32], &[2; 16]);

        let register = client.register(&session, "Ada", &FixedSigner, 5);
        let claim = claim_of(&register).expect("a registration");
        assert_eq!(claim.requested_username, "Ada");
        assert_eq!(claim.device_public_key, vec![7; 32]);
        assert_eq!(register.sent_at_unix_ms, 5);
        assert_eq!(register.validate(), Ok(()));

        let authenticate = client.authenticate(&session, &FixedSigner, 6);
        let proof = proof_of(&authenticate).expect("a proof");
        assert_eq!(proof.device_public_key, vec![7; 32]);
        assert_eq!(authenticate.validate(), Ok(()));
        // Distinct operations must not produce interchangeable signatures.
        assert_ne!(claim.signature, proof.signature);
        assert!(claim_of(&authenticate).is_none());
        assert!(proof_of(&register).is_none());
    }

    #[test]
    fn reads_a_confirmed_identity() {
        let request = ping_envelope(1);

        let identity = validate_authenticated(&request, &authenticated_envelope(&request, 1))
            .expect("a correlated confirmation");

        assert_eq!(identity.username, "Ada");
        assert_eq!(identity.discriminator, "7Q2XZ");
    }

    #[test]
    fn rejects_uncorrelated_unexpected_or_mismatched_confirmations() {
        let request = ping_envelope(1);

        let mut stray = authenticated_envelope(&request, 1);
        stray.correlation_id = new_message_id();
        assert_eq!(
            validate_authenticated(&request, &stray),
            Err(ClientError::InvalidCorrelation)
        );

        assert_eq!(
            validate_authenticated(&request, &authenticated_envelope(&request, 99)),
            Err(ClientError::UnsupportedProtocolVersion)
        );

        let mut wrong_payload = authenticated_envelope(&request, 1);
        wrong_payload.payload = Some(Payload::Pong(Pong { nonce: 1 }));
        assert_eq!(
            validate_authenticated(&request, &wrong_payload),
            Err(ClientError::UnexpectedEnvelope {
                expected: "Authenticated"
            })
        );
    }

    #[test]
    fn surfaces_a_typed_refusal_from_the_server() {
        let request = ping_envelope(1);
        let mut refused = authenticated_envelope(&request, 1);
        refused.payload = Some(Payload::ProtocolError(
            portalis_nexus_protocol::v1::ProtocolError {
                code: ProtocolErrorCode::Unauthorized as i32,
                message: "this device was revoked".to_owned(),
                retry_after_ms: None,
                retryable: false,
            },
        ));

        assert_eq!(
            validate_authenticated(&request, &refused),
            Err(ClientError::Refused {
                code: ProtocolErrorCode::Unauthorized,
                message: "this device was revoked".to_owned(),
            })
        );

        // An unknown code degrades to unspecified rather than failing to parse.
        let mut unknown = refused.clone();
        unknown.payload = Some(Payload::ProtocolError(
            portalis_nexus_protocol::v1::ProtocolError {
                code: 9_999,
                message: "from a newer server".to_owned(),
                retry_after_ms: None,
                retryable: false,
            },
        ));
        assert_eq!(
            validate_authenticated(&request, &unknown),
            Err(ClientError::Refused {
                code: ProtocolErrorCode::Unspecified,
                message: "from a newer server".to_owned(),
            })
        );
    }
}
