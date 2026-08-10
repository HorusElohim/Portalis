//! Deterministic client-side protocol rules, with no sockets involved.

use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    AuthenticateDevice, Authenticated, DeviceLinked, Envelope, Friend, FriendAction, FriendCommand,
    KeyEnvelope, KeyEnvelopePut, LinkDevice, ListFriendsRequest, ListFriendsResponse,
    ListKeyEnvelopesRequest, ListKeyEnvelopesResponse, Ping, Pong, ProtocolErrorCode,
    PutKeyEnvelope, RegisterUser, ResolveHandleRequest, ResolveHandleResponse, ServerHello,
};
use portalis_nexus_protocol::{
    CURRENT_PROTOCOL_VERSION, SHARE_ID_BYTES, SessionBinding, authentication_payload,
    link_device_payload, new_message_id, registration_payload, validate_server_hello,
};

use crate::error::ClientError;
use crate::signer::DeviceSigner;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientProtocol {
    version: u32,
}

/// One bounded page of key envelopes addressed to the authenticated device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEnvelopePage {
    pub envelopes: Vec<KeyEnvelope>,
    pub next_after_share_id: Option<[u8; SHARE_ID_BYTES]>,
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
        timestamp_unix_ns: u64,
    ) -> Envelope {
        let public_key = signer.public_key();
        let encryption_public_key = signer.encryption_public_key();
        let payload = registration_payload(binding, username, &public_key, &encryption_public_key);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns,
            payload: Some(Payload::RegisterUser(RegisterUser {
                requested_username: username.to_owned(),
                device_public_key: public_key.to_vec(),
                encryption_public_key: encryption_public_key.to_vec(),
                signature: signer.sign(&payload).to_vec(),
            })),
        }
    }

    /// Builds an approval, signed by this device, for a new device's keys.
    ///
    /// Unlike registration and authentication this carries no
    /// [`SessionBinding`]: the approval is durable, valid for this server
    /// however many times and on whatever connection the candidate device
    /// eventually submits it.
    #[must_use]
    pub fn link_device<S: DeviceSigner + ?Sized>(
        &self,
        server_authority: &str,
        candidate_signing_public_key: &[u8],
        candidate_encryption_public_key: &[u8],
        approver: &S,
        timestamp_unix_ns: u64,
    ) -> Envelope {
        let payload = link_device_payload(
            server_authority,
            candidate_signing_public_key,
            candidate_encryption_public_key,
        );
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns,
            payload: Some(Payload::LinkDevice(LinkDevice {
                candidate_signing_public_key: candidate_signing_public_key.to_vec(),
                candidate_encryption_public_key: candidate_encryption_public_key.to_vec(),
                approval_signature: approver.sign(&payload).to_vec(),
            })),
        }
    }

    /// Builds a signed request to prove this device is enrolled.
    #[must_use]
    pub fn authenticate<S: DeviceSigner + ?Sized>(
        &self,
        binding: &SessionBinding<'_>,
        signer: &S,
        timestamp_unix_ns: u64,
    ) -> Envelope {
        let public_key = signer.public_key();
        let payload = authentication_payload(binding, &public_key);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns,
            payload: Some(Payload::AuthenticateDevice(AuthenticateDevice {
                device_public_key: public_key.to_vec(),
                signature: signer.sign(&payload).to_vec(),
            })),
        }
    }

    /// Builds a request for the user behind a handle.
    #[must_use]
    pub fn resolve_handle(&self, handle: &str, timestamp_unix_ns: u64) -> Envelope {
        Self::envelope(
            Payload::ResolveHandleRequest(ResolveHandleRequest {
                handle: handle.to_owned(),
            }),
            timestamp_unix_ns,
        )
    }

    /// Builds a friend action against `peer`.
    #[must_use]
    pub fn friend_command(
        &self,
        action: FriendAction,
        peer: &[u8],
        timestamp_unix_ns: u64,
    ) -> Envelope {
        Self::envelope(
            Payload::FriendCommand(FriendCommand {
                action: action as i32,
                peer_user_id: peer.to_vec(),
            }),
            timestamp_unix_ns,
        )
    }

    /// Builds a request to store a sealed share key for one of this user's
    /// own devices.
    ///
    /// The caller seals the key itself with
    /// [`portalis_nexus_protocol::seal`]; this only carries the result.
    #[must_use]
    pub fn put_key_envelope(
        &self,
        share_id: &[u8],
        recipient_device_id: &[u8],
        ephemeral_public_key: &[u8],
        ciphertext: &[u8],
        timestamp_unix_ns: u64,
    ) -> Envelope {
        Self::envelope(
            Payload::PutKeyEnvelope(PutKeyEnvelope {
                share_id: share_id.to_vec(),
                recipient_device_id: recipient_device_id.to_vec(),
                ephemeral_public_key: ephemeral_public_key.to_vec(),
                ciphertext: ciphertext.to_vec(),
            }),
            timestamp_unix_ns,
        )
    }

    /// Builds a request for one page of envelopes addressed to this device.
    #[must_use]
    pub fn list_key_envelopes(
        &self,
        after_share_id: Option<&[u8]>,
        timestamp_unix_ns: u64,
    ) -> Envelope {
        Self::envelope(
            Payload::ListKeyEnvelopesRequest(ListKeyEnvelopesRequest {
                after_share_id: after_share_id.unwrap_or_default().to_vec(),
            }),
            timestamp_unix_ns,
        )
    }

    /// Builds a request for every friendship this user is part of.
    #[must_use]
    pub fn list_friends(&self, timestamp_unix_ns: u64) -> Envelope {
        Self::envelope(
            Payload::ListFriendsRequest(ListFriendsRequest {}),
            timestamp_unix_ns,
        )
    }

    fn envelope(payload: Payload, timestamp_unix_ns: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns,
            payload: Some(payload),
        }
    }

    #[must_use]
    pub fn ping(&self, nonce: u64, timestamp_unix_ns: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns,
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

/// Reads the payload a server answered with, turning a refusal into an error.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not correlated to the request, or
/// when the server refused it.
pub fn validate_reply<'a>(
    request: &Envelope,
    response: &'a Envelope,
) -> Result<&'a Payload, ClientError> {
    if response.correlation_id != request.message_id {
        return Err(ClientError::InvalidCorrelation);
    }
    match &response.payload {
        Some(Payload::ProtocolError(refusal)) => Err(ClientError::Refused {
            code: ProtocolErrorCode::try_from(refusal.code)
                .unwrap_or(ProtocolErrorCode::Unspecified),
            message: refusal.message.clone(),
        }),
        Some(payload) => Ok(payload),
        None => Err(ClientError::UnexpectedEnvelope {
            expected: "a payload",
        }),
    }
}

/// Reads a resolved handle.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not a resolved handle.
pub fn validate_resolved(
    request: &Envelope,
    response: &Envelope,
) -> Result<ResolveHandleResponse, ClientError> {
    match validate_reply(request, response)? {
        Payload::ResolveHandleResponse(resolved) => Ok(resolved.clone()),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "ResolveHandleResponse",
        }),
    }
}

/// Reads a stored-envelope confirmation.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not one.
pub fn validate_key_envelope_put(
    request: &Envelope,
    response: &Envelope,
) -> Result<KeyEnvelopePut, ClientError> {
    match validate_reply(request, response)? {
        Payload::KeyEnvelopePut(stored) => Ok(stored.clone()),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "KeyEnvelopePut",
        }),
    }
}

/// Reads the envelopes addressed to this device.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not an envelope list.
pub fn validate_key_envelopes(
    request: &Envelope,
    response: &Envelope,
) -> Result<KeyEnvelopePage, ClientError> {
    match validate_reply(request, response)? {
        Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
            envelopes,
            next_after_share_id,
        }) => Ok(KeyEnvelopePage {
            envelopes: envelopes.clone(),
            next_after_share_id: (!next_after_share_id.is_empty())
                .then(|| {
                    next_after_share_id.as_slice().try_into().map_err(|_| {
                        ClientError::InvalidField {
                            field: "next_after_share_id",
                        }
                    })
                })
                .transpose()?,
        }),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "ListKeyEnvelopesResponse",
        }),
    }
}

/// Reads a device-linked confirmation.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not a device-linked confirmation.
pub fn validate_device_linked(
    request: &Envelope,
    response: &Envelope,
) -> Result<DeviceLinked, ClientError> {
    match validate_reply(request, response)? {
        Payload::DeviceLinked(linked) => Ok(linked.clone()),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "DeviceLinked",
        }),
    }
}

/// Reads the friendship a command produced.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not a friend event.
pub fn validate_friend_event(
    request: &Envelope,
    response: &Envelope,
) -> Result<Friend, ClientError> {
    match validate_reply(request, response)? {
        Payload::FriendEvent(event) => {
            event.friend.clone().ok_or(ClientError::UnexpectedEnvelope {
                expected: "a friend",
            })
        }
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "FriendEvent",
        }),
    }
}

/// Reads a friend list.
///
/// # Errors
///
/// Returns [`ClientError`] when the reply is not a friend list.
pub fn validate_friend_list(
    request: &Envelope,
    response: &Envelope,
) -> Result<Vec<Friend>, ClientError> {
    match validate_reply(request, response)? {
        Payload::ListFriendsResponse(ListFriendsResponse { friends }) => Ok(friends.clone()),
        _ => Err(ClientError::UnexpectedEnvelope {
            expected: "ListFriendsResponse",
        }),
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::new_challenge;
    use portalis_nexus_protocol::v1::FriendEvent;
    use portalis_nexus_protocol::v1::ProtocolRange;

    use super::*;

    fn hello_envelope(range: ProtocolRange) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: 1,
            payload: Some(Payload::ServerHello(ServerHello {
                connection_id: new_message_id(),
                challenge: new_challenge(),
                server_time_unix_ns: 1,
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
            timestamp_unix_ns: 1,
            payload: Some(Payload::Pong(Pong { nonce })),
        }
    }

    #[test]
    fn builds_valid_ping_for_current_protocol() {
        let client = ClientProtocol::default();
        let envelope = client.ping(42, 1000);

        assert_eq!(client.version(), CURRENT_PROTOCOL_VERSION);
        assert_eq!(client, client.clone());
        assert!(!format!("{client:?}").is_empty());
        assert_eq!(envelope.timestamp_unix_ns, 1000);
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
            timestamp_unix_ns: 1,
            payload: Some(Payload::ServerHello(ServerHello {
                connection_id: vec![0; 15],
                challenge: new_challenge(),
                server_time_unix_ns: 1,
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
            timestamp_unix_ns: 1,
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

    fn command_of(envelope: &Envelope) -> Option<FriendCommand> {
        match &envelope.payload {
            Some(Payload::FriendCommand(command)) => Some(command.clone()),
            _ => None,
        }
    }

    fn proof_of(envelope: &Envelope) -> Option<AuthenticateDevice> {
        match &envelope.payload {
            Some(Payload::AuthenticateDevice(proof)) => Some(proof.clone()),
            _ => None,
        }
    }

    fn link_of(envelope: &Envelope) -> Option<LinkDevice> {
        match &envelope.payload {
            Some(Payload::LinkDevice(link)) => Some(link.clone()),
            _ => None,
        }
    }

    struct FixedSigner;

    impl DeviceSigner for FixedSigner {
        fn public_key(&self) -> [u8; 32] {
            [7; 32]
        }

        fn encryption_public_key(&self) -> [u8; 32] {
            [8; 32]
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
            server_time_unix_ns: 1,
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
        assert_eq!(claim.encryption_public_key, vec![8; 32]);
        assert_eq!(register.timestamp_unix_ns, 5);
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
    fn builds_a_durable_link_device_approval() {
        let client = ClientProtocol::default();
        let candidate_signing_public_key = [9; 32];
        let candidate_encryption_public_key = [10; 32];

        // Deliberately no `SessionBinding` here, unlike registration and
        // authentication: a link approval is not tied to a connection.
        let request = client.link_device(
            "nexus.portalis.test",
            &candidate_signing_public_key,
            &candidate_encryption_public_key,
            &FixedSigner,
            7,
        );

        let link = link_of(&request).expect("a link-device request");
        assert_eq!(link.candidate_signing_public_key, vec![9; 32]);
        assert_eq!(link.candidate_encryption_public_key, vec![10; 32]);
        assert_eq!(request.timestamp_unix_ns, 7);
        assert_eq!(request.validate(), Ok(()));
        assert!(link_of(&ping_envelope(1)).is_none());
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
    fn reads_a_device_linked_confirmation() {
        let request = ping_envelope(1);
        let response = Envelope {
            message_id: new_message_id(),
            correlation_id: request.message_id.clone(),
            timestamp_unix_ns: 2,
            payload: Some(Payload::DeviceLinked(DeviceLinked {
                user_id: vec![1; 16],
                device_id: vec![2; 32],
            })),
        };

        let linked =
            validate_device_linked(&request, &response).expect("a device-linked confirmation");
        assert_eq!(linked.user_id, vec![1; 16]);

        let mut wrong_payload = response;
        wrong_payload.payload = Some(Payload::Pong(Pong { nonce: 1 }));
        assert_eq!(
            validate_device_linked(&request, &wrong_payload),
            Err(ClientError::UnexpectedEnvelope {
                expected: "DeviceLinked"
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

    fn resolved_envelope(request: &Envelope) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: request.message_id.clone(),
            timestamp_unix_ns: 1,
            payload: Some(Payload::ResolveHandleResponse(ResolveHandleResponse {
                user_id: vec![2; 16],
                username: "Grace".to_owned(),
                discriminator: "ABCDE".to_owned(),
            })),
        }
    }

    fn answered_with(request: &Envelope, payload: Payload) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: request.message_id.clone(),
            timestamp_unix_ns: 1,
            payload: Some(payload),
        }
    }

    #[test]
    fn builds_the_key_envelope_commands() {
        let client = ClientProtocol::default();

        let put = client.put_key_envelope(&[3; SHARE_ID_BYTES], &[1; 32], &[9; 32], b"sealed", 5);
        assert_eq!(put.timestamp_unix_ns, 5);
        assert_eq!(put.validate(), Ok(()));
        assert_eq!(
            put.payload,
            Some(Payload::PutKeyEnvelope(PutKeyEnvelope {
                share_id: vec![3; SHARE_ID_BYTES],
                recipient_device_id: vec![1; 32],
                ephemeral_public_key: vec![9; 32],
                ciphertext: b"sealed".to_vec(),
            }))
        );

        // No cursor means the first page, carried as an empty field rather
        // than a missing one.
        let first = client.list_key_envelopes(None, 6);
        assert_eq!(
            first.payload,
            Some(Payload::ListKeyEnvelopesRequest(ListKeyEnvelopesRequest {
                after_share_id: Vec::new(),
            }))
        );
        let resumed = client.list_key_envelopes(Some(&[3; SHARE_ID_BYTES]), 7);
        assert_eq!(
            resumed.payload,
            Some(Payload::ListKeyEnvelopesRequest(ListKeyEnvelopesRequest {
                after_share_id: vec![3; SHARE_ID_BYTES],
            }))
        );
        assert_eq!(resumed.validate(), Ok(()));
    }

    #[test]
    fn reads_a_stored_key_envelope_confirmation() {
        let client = ClientProtocol::default();
        let request = client.put_key_envelope(&[3; SHARE_ID_BYTES], &[1; 32], &[9; 32], b"c", 5);

        let stored = answered_with(
            &request,
            Payload::KeyEnvelopePut(KeyEnvelopePut {
                share_id: vec![3; SHARE_ID_BYTES],
                recipient_device_id: vec![1; 32],
            }),
        );
        assert_eq!(
            validate_key_envelope_put(&request, &stored),
            Ok(KeyEnvelopePut {
                share_id: vec![3; SHARE_ID_BYTES],
                recipient_device_id: vec![1; 32],
            })
        );

        // An answer of the wrong shape is not a confirmation.
        let wrong = answered_with(&request, Payload::FriendEvent(FriendEvent { friend: None }));
        assert_eq!(
            validate_key_envelope_put(&request, &wrong),
            Err(ClientError::UnexpectedEnvelope {
                expected: "KeyEnvelopePut",
            })
        );
    }

    #[test]
    fn reads_a_page_of_key_envelopes() {
        let client = ClientProtocol::default();
        let request = client.list_key_envelopes(None, 5);
        let envelope = KeyEnvelope {
            share_id: vec![3; SHARE_ID_BYTES],
            ephemeral_public_key: vec![9; 32],
            ciphertext: b"sealed".to_vec(),
        };

        // A full page carries the cursor the next request resumes from.
        let page = answered_with(
            &request,
            Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
                envelopes: vec![envelope.clone()],
                next_after_share_id: vec![3; SHARE_ID_BYTES],
            }),
        );
        assert_eq!(
            validate_key_envelopes(&request, &page),
            Ok(KeyEnvelopePage {
                envelopes: vec![envelope.clone()],
                next_after_share_id: Some([3; SHARE_ID_BYTES]),
            })
        );

        // A last page carries no cursor, which is an empty field on the wire.
        let last = answered_with(
            &request,
            Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
                envelopes: vec![envelope.clone()],
                next_after_share_id: Vec::new(),
            }),
        );
        assert_eq!(
            validate_key_envelopes(&request, &last),
            Ok(KeyEnvelopePage {
                envelopes: vec![envelope],
                next_after_share_id: None,
            })
        );
    }

    #[test]
    fn rejects_a_key_envelope_page_that_cannot_be_resumed() {
        let client = ClientProtocol::default();
        let request = client.list_key_envelopes(None, 5);

        // A cursor that is not a share ID would be sent back as one.
        let truncated = answered_with(
            &request,
            Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
                envelopes: Vec::new(),
                next_after_share_id: vec![3; SHARE_ID_BYTES - 1],
            }),
        );
        assert_eq!(
            validate_key_envelopes(&request, &truncated),
            Err(ClientError::InvalidField {
                field: "next_after_share_id",
            })
        );

        let wrong = answered_with(&request, Payload::FriendEvent(FriendEvent { friend: None }));
        assert_eq!(
            validate_key_envelopes(&request, &wrong),
            Err(ClientError::UnexpectedEnvelope {
                expected: "ListKeyEnvelopesResponse",
            })
        );
    }

    #[test]
    fn builds_the_friend_commands() {
        let client = ClientProtocol::default();

        let lookup = client.resolve_handle("grace#ABCDE", 5);
        assert_eq!(lookup.timestamp_unix_ns, 5);
        assert_eq!(lookup.validate(), Ok(()));
        assert_eq!(
            lookup.payload,
            Some(Payload::ResolveHandleRequest(ResolveHandleRequest {
                handle: "grace#ABCDE".to_owned()
            }))
        );

        let command = client.friend_command(FriendAction::Accept, &[2; 16], 6);
        let action = command_of(&command).expect("a friend command");
        assert_eq!(action.action(), FriendAction::Accept);
        assert_eq!(action.peer_user_id, vec![2; 16]);
        assert!(command_of(&lookup).is_none());

        let listing = client.list_friends(7);
        assert_eq!(
            listing.payload,
            Some(Payload::ListFriendsRequest(ListFriendsRequest {}))
        );
        assert_eq!(listing.validate(), Ok(()));
    }

    #[test]
    fn reads_the_answers_to_friend_commands() {
        let request = ping_envelope(1);

        assert_eq!(
            validate_resolved(&request, &resolved_envelope(&request))
                .expect("resolved")
                .username,
            "Grace"
        );

        let friend = Friend {
            user_id: vec![2; 16],
            username: "Grace".to_owned(),
            discriminator: "ABCDE".to_owned(),
            state: 3,
            requested_by_me: true,
        };
        let event = answered_with(
            &request,
            Payload::FriendEvent(FriendEvent {
                friend: Some(friend.clone()),
            }),
        );
        assert_eq!(validate_friend_event(&request, &event), Ok(friend.clone()));

        let listing = answered_with(
            &request,
            Payload::ListFriendsResponse(ListFriendsResponse {
                friends: vec![friend.clone()],
            }),
        );
        assert_eq!(validate_friend_list(&request, &listing), Ok(vec![friend]));
    }

    #[test]
    fn rejects_answers_that_do_not_fit_the_question() {
        let request = ping_envelope(1);
        let pong = answered_with(&request, Payload::Pong(Pong { nonce: 1 }));

        assert_eq!(
            validate_resolved(&request, &pong),
            Err(ClientError::UnexpectedEnvelope {
                expected: "ResolveHandleResponse"
            })
        );
        assert_eq!(
            validate_friend_event(&request, &pong),
            Err(ClientError::UnexpectedEnvelope {
                expected: "FriendEvent"
            })
        );
        assert_eq!(
            validate_friend_list(&request, &pong),
            Err(ClientError::UnexpectedEnvelope {
                expected: "ListFriendsResponse"
            })
        );

        // A friend event with no friend in it is not an answer either.
        let empty = answered_with(&request, Payload::FriendEvent(FriendEvent { friend: None }));
        assert_eq!(
            validate_friend_event(&request, &empty),
            Err(ClientError::UnexpectedEnvelope {
                expected: "a friend"
            })
        );
    }

    #[test]
    fn a_reply_must_be_correlated_answered_and_not_a_refusal() {
        let request = ping_envelope(1);

        let mut stray = resolved_envelope(&request);
        stray.correlation_id = new_message_id();
        assert_eq!(
            validate_reply(&request, &stray),
            Err(ClientError::InvalidCorrelation)
        );

        let mut empty = resolved_envelope(&request);
        empty.payload = None;
        assert_eq!(
            validate_reply(&request, &empty),
            Err(ClientError::UnexpectedEnvelope {
                expected: "a payload"
            })
        );

        let refused = answered_with(
            &request,
            Payload::ProtocolError(portalis_nexus_protocol::v1::ProtocolError {
                code: ProtocolErrorCode::RateLimited as i32,
                message: "try again".to_owned(),
                retry_after_ms: None,
                retryable: true,
            }),
        );
        assert_eq!(
            validate_reply(&request, &refused),
            Err(ClientError::Refused {
                code: ProtocolErrorCode::RateLimited,
                message: "try again".to_owned(),
            })
        );
    }
}
