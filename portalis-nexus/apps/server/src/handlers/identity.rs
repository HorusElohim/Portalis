//! Registration and device authentication commands.

use portalis_nexus_protocol::v1::{AuthenticateDevice, Envelope, ProtocolErrorCode, RegisterUser};
use portalis_nexus_server_core::{
    AuthenticationRequest, ChallengeError, Identity, IdentityError, IdentityRepository,
    RegistrationRequest,
};

use crate::identity::NexusIdentities;
use crate::messages::{authenticated_reply, protocol_error};
use crate::session::Session;

/// Claims a username and enrols the signing device as its first.
pub(crate) async fn claim<S: IdentityRepository>(
    session: &mut Session,
    identities: &NexusIdentities<S>,
    server_authority: &str,
    request: &Envelope,
    register: &RegisterUser,
    now_unix_ms: u64,
) -> Envelope {
    if let Err(error) = session.spend(&register.signature, now_unix_ms) {
        return challenge_rejection(request, &error, now_unix_ms);
    }
    let outcome = identities
        .register(RegistrationRequest {
            binding: session.binding(server_authority),
            requested_username: &register.requested_username,
            device_public_key: &register.device_public_key,
            signature: &register.signature,
        })
        .await;
    settle(session, request, outcome, now_unix_ms)
}

/// Proves this connection holds the key of an authorized device.
pub(crate) async fn prove<S: IdentityRepository>(
    session: &mut Session,
    identities: &NexusIdentities<S>,
    server_authority: &str,
    request: &Envelope,
    authenticate: &AuthenticateDevice,
    now_unix_ms: u64,
) -> Envelope {
    if let Err(error) = session.spend(&authenticate.signature, now_unix_ms) {
        return challenge_rejection(request, &error, now_unix_ms);
    }
    let outcome = identities
        .authenticate(AuthenticationRequest {
            binding: session.binding(server_authority),
            device_public_key: &authenticate.device_public_key,
            signature: &authenticate.signature,
        })
        .await;
    settle(session, request, outcome, now_unix_ms)
}

/// Binds the connection on success, or explains the refusal.
fn settle(
    session: &mut Session,
    request: &Envelope,
    outcome: Result<Identity, IdentityError>,
    now_unix_ms: u64,
) -> Envelope {
    match outcome {
        Ok(identity) => {
            let reply = authenticated_reply(request, &identity, now_unix_ms);
            session.bind(identity);
            reply
        }
        Err(error) => identity_rejection(request, &error, now_unix_ms),
    }
}

/// Maps a challenge failure onto the wire.
fn challenge_rejection(request: &Envelope, error: &ChallengeError, now_unix_ms: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        error.to_string(),
        now_unix_ms,
    )
}

/// Maps an identity failure onto the wire, without revealing more than the
/// caller already knows.
fn identity_rejection(request: &Envelope, error: &IdentityError, now_unix_ms: u64) -> Envelope {
    let code = match error {
        IdentityError::Signature(_) | IdentityError::UnknownDevice => {
            ProtocolErrorCode::Unauthenticated
        }
        IdentityError::DeviceRevoked => ProtocolErrorCode::Unauthorized,
        IdentityError::Handle(_)
        | IdentityError::UsernameUnavailable
        | IdentityError::DeviceAlreadyRegistered => ProtocolErrorCode::InvalidMessage,
        IdentityError::Repository(_) | IdentityError::MissingUser => ProtocolErrorCode::Internal,
    };
    // Storage detail stays in the server's logs, not on the wire.
    let message = match error {
        IdentityError::Repository(_) | IdentityError::MissingUser => {
            "the identity store is unavailable".to_owned()
        }
        other => other.to_string(),
    };
    protocol_error(code, request.message_id.clone(), message, now_unix_ms)
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
    use portalis_nexus_protocol::SignatureError;
    use portalis_nexus_protocol::v1::envelope::Payload;
    use portalis_nexus_protocol::v1::{Authenticated, Ping, Pong, ProtocolError, ServerHello};
    use portalis_nexus_protocol::{
        CHALLENGE_LIFETIME_MS, SessionBinding, authentication_payload, derive_device_id,
        new_message_id, registration_payload,
    };
    use portalis_nexus_server_core::HandleError;
    use portalis_nexus_server_core::{InMemoryIdentities, ProtocolPolicy, RepositoryError};

    use super::*;
    use crate::handlers::dispatch;
    use crate::identity::identities;
    use crate::messages::hello_payload;

    const NOW: u64 = 1_700_000_000_000;
    const AUTHORITY: &str = "nexus.portalis.test";

    fn store() -> NexusIdentities<InMemoryIdentities> {
        identities(InMemoryIdentities::default())
    }

    /// The message a reply carries, or `None` when it is not a refusal.
    fn refusal_message(reply: &Envelope) -> Option<String> {
        match &reply.payload {
            Some(Payload::ProtocolError(ProtocolError { message, .. })) => Some(message.clone()),
            _ => None,
        }
    }

    fn greeting() -> ServerHello {
        let policy = ProtocolPolicy::new(CURRENT_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION)
            .expect("valid protocol range");
        hello_payload(&policy, NOW)
    }

    fn key(seed: u8) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
    }

    fn binding_for(hello: &ServerHello) -> SessionBinding<'_> {
        SessionBinding {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            server_authority: AUTHORITY,
            connection_id: &hello.connection_id,
            challenge: &hello.challenge,
            server_time_unix_ms: NOW,
        }
    }

    fn register_envelope(
        hello: &ServerHello,
        username: &str,
        signer: &ed25519_dalek::SigningKey,
    ) -> Envelope {
        use ed25519_dalek::Signer as _;
        let public = signer.verifying_key().to_bytes();
        let payload = registration_payload(&binding_for(hello), username, &public);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::RegisterUser(RegisterUser {
                requested_username: username.to_owned(),
                device_public_key: public.to_vec(),
                signature: signer.sign(&payload).to_bytes().to_vec(),
            })),
        }
    }

    fn authenticate_envelope(hello: &ServerHello, signer: &ed25519_dalek::SigningKey) -> Envelope {
        use ed25519_dalek::Signer as _;
        let public = signer.verifying_key().to_bytes();
        let payload = authentication_payload(&binding_for(hello), &public);
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::AuthenticateDevice(AuthenticateDevice {
                device_public_key: public.to_vec(),
                signature: signer.sign(&payload).to_bytes().to_vec(),
            })),
        }
    }

    /// The identity in a reply, or `None` when it confirmed none.
    fn authenticated(reply: &Envelope) -> Option<Authenticated> {
        match &reply.payload {
            Some(Payload::Authenticated(identity)) => Some(identity.clone()),
            _ => None,
        }
    }

    /// The refusal code in a reply, or `None` when it is not a refusal.
    fn refusal(reply: &Envelope) -> Option<ProtocolErrorCode> {
        let Some(Payload::ProtocolError(ProtocolError { code, .. })) = &reply.payload else {
            return None;
        };
        ProtocolErrorCode::try_from(*code).ok()
    }

    #[tokio::test]
    async fn registering_binds_the_connection_to_its_identity() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        assert!(!session.is_authenticated());
        assert!(session.identity().is_none());
        let request = register_envelope(&hello, "Ada", &key(7));

        let reply = dispatch(&mut session, &identities, AUTHORITY, &request, NOW + 1).await;

        let identity = authenticated(&reply).expect("an Authenticated reply");
        assert_eq!(identity.username, "Ada");
        assert_eq!(reply.correlation_id, request.message_id);
        assert!(session.is_authenticated());
        assert_eq!(
            session.identity().expect("bound").device.device_id,
            derive_device_id(&key(7).verifying_key().to_bytes())
        );
    }

    #[tokio::test]
    async fn a_challenge_is_spent_once_even_on_a_second_command() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &register_envelope(&hello, "Ada", &key(7)),
            NOW,
        )
        .await;

        let reply = dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &authenticate_envelope(&hello, &key(7)),
            NOW,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthenticated));
    }

    #[tokio::test]
    async fn an_expired_challenge_is_refused() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);

        let reply = dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &register_envelope(&hello, "Ada", &key(7)),
            NOW + CHALLENGE_LIFETIME_MS + 1,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthenticated));
        assert!(!session.is_authenticated());
    }

    #[tokio::test]
    async fn an_unsigned_command_is_refused_without_spending_the_challenge() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        let request = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::RegisterUser(RegisterUser {
                requested_username: "Ada".to_owned(),
                device_public_key: key(7).verifying_key().to_bytes().to_vec(),
                signature: Vec::new(),
            })),
        };

        let reply = dispatch(&mut session, &identities, AUTHORITY, &request, NOW).await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthenticated));
        // The challenge survives, so an honest retry still works.
        let retry = dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &register_envelope(&hello, "Ada", &key(7)),
            NOW,
        )
        .await;
        assert!(authenticated(&retry).is_some());
    }

    #[tokio::test]
    async fn a_signature_for_another_server_is_refused() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);

        let reply = dispatch(
            &mut session,
            &identities,
            "nexus.attacker.test",
            &register_envelope(&hello, "Ada", &key(7)),
            NOW,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthenticated));
    }

    #[tokio::test]
    async fn a_revoked_device_is_unauthorized() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &register_envelope(&hello, "Ada", &key(7)),
            NOW,
        )
        .await;
        identities
            .revoke_device(derive_device_id(&key(7).verifying_key().to_bytes()))
            .await
            .expect("revocation succeeds");

        // A fresh connection carries a fresh challenge.
        let next_hello = greeting();
        let mut next = Session::new(&next_hello);
        let reply = dispatch(
            &mut next,
            &identities,
            AUTHORITY,
            &authenticate_envelope(&next_hello, &key(7)),
            NOW,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthorized));
    }

    #[tokio::test]
    async fn registering_a_device_twice_is_rejected_as_invalid() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &register_envelope(&hello, "Ada", &key(7)),
            NOW,
        )
        .await;

        let next_hello = greeting();
        let mut next = Session::new(&next_hello);
        let reply = dispatch(
            &mut next,
            &identities,
            AUTHORITY,
            &register_envelope(&next_hello, "Grace", &key(7)),
            NOW,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::InvalidMessage));
    }

    #[tokio::test]
    async fn an_unknown_device_is_unauthenticated() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);

        let reply = dispatch(
            &mut session,
            &identities,
            AUTHORITY,
            &authenticate_envelope(&hello, &key(11)),
            NOW,
        )
        .await;

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Unauthenticated));
        assert!(authenticated(&reply).is_none());
    }

    #[tokio::test]
    async fn non_identity_requests_fall_through_to_the_stateless_reply() {
        let identities = store();
        let hello = greeting();
        let mut session = Session::new(&hello);
        let ping = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 5 })),
        };

        let reply = dispatch(&mut session, &identities, AUTHORITY, &ping, NOW).await;

        assert_eq!(reply.payload, Some(Payload::Pong(Pong { nonce: 5 })));
        assert!(refusal(&reply).is_none());
    }

    #[test]
    fn maps_every_identity_failure_onto_a_typed_refusal() {
        let request = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        };

        for (error, expected) in [
            (
                IdentityError::Signature(SignatureError::Rejected),
                ProtocolErrorCode::Unauthenticated,
            ),
            (
                IdentityError::UnknownDevice,
                ProtocolErrorCode::Unauthenticated,
            ),
            (
                IdentityError::DeviceRevoked,
                ProtocolErrorCode::Unauthorized,
            ),
            (
                IdentityError::Handle(HandleError::UsernameCharset),
                ProtocolErrorCode::InvalidMessage,
            ),
            (
                IdentityError::UsernameUnavailable,
                ProtocolErrorCode::InvalidMessage,
            ),
            (
                IdentityError::DeviceAlreadyRegistered,
                ProtocolErrorCode::InvalidMessage,
            ),
            (IdentityError::MissingUser, ProtocolErrorCode::Internal),
        ] {
            let reply = identity_rejection(&request, &error, NOW);

            assert_eq!(refusal(&reply), Some(expected), "for {error}");
            assert_eq!(reply.correlation_id, request.message_id);
        }
    }

    #[test]
    fn storage_detail_never_reaches_the_wire() {
        let request = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        };
        let outage = IdentityError::Repository(RepositoryError::Unavailable(
            "connection refused to db-1.internal".to_owned(),
        ));

        let reply = identity_rejection(&request, &outage, NOW);

        assert_eq!(refusal(&reply), Some(ProtocolErrorCode::Internal));
        let message = refusal_message(&reply).expect("a refusal message");
        assert_eq!(message, "the identity store is unavailable");
        assert!(
            !message.contains("db-1.internal"),
            "storage detail belongs in the logs, not on the wire"
        );
        assert!(refusal_message(&request).is_none());
    }
}
