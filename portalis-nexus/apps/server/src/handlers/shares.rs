//! Key-envelope push and fetch.

use portalis_nexus_protocol::SHARE_ID_BYTES;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, KeyEnvelope, KeyEnvelopePut, ListKeyEnvelopesRequest, ListKeyEnvelopesResponse,
    ProtocolErrorCode, PutKeyEnvelope,
};
use portalis_nexus_server_core::{
    DeviceId, EnvelopeError, KeyEnvelopeRecord, PutKeyEnvelopeRequest,
};

use crate::identity::{DefaultStore, NexusEnvelopes};
use crate::messages::{protocol_error, reply_with};
use crate::session::Session;

/// Stores a sealed share key for one of this user's own devices.
pub(crate) async fn put(
    session: &Session,
    envelopes: &NexusEnvelopes<DefaultStore>,
    request: &Envelope,
    put: &PutKeyEnvelope,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(put.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ns);
    };
    let Ok(recipient_device_id) = DeviceId::try_from(put.recipient_device_id.as_slice()) else {
        return malformed(
            request,
            "recipient_device_id must name a device",
            now_unix_ns,
        );
    };

    let outcome = envelopes
        .put_key_envelope(
            identity.user.user_id,
            PutKeyEnvelopeRequest {
                share_id,
                recipient_device_id,
                ephemeral_public_key: &put.ephemeral_public_key,
                ciphertext: &put.ciphertext,
            },
        )
        .await;
    match outcome {
        Ok(()) => reply_with(
            request,
            Payload::KeyEnvelopePut(KeyEnvelopePut {
                share_id: share_id.to_vec(),
                recipient_device_id: recipient_device_id.to_vec(),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

/// Returns every envelope addressed to this connection's own device.
///
/// A connection only ever asks for its own: the device is taken from the
/// authenticated session rather than the request, so there is nothing to
/// authorize beyond having authenticated at all.
pub(crate) async fn list(
    session: &Session,
    envelopes: &NexusEnvelopes<DefaultStore>,
    request: &Envelope,
    list: &ListKeyEnvelopesRequest,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let after_share_id = if list.after_share_id.is_empty() {
        None
    } else {
        let Ok(after_share_id) = <[u8; SHARE_ID_BYTES]>::try_from(list.after_share_id.as_slice())
        else {
            return malformed(request, "after_share_id must name a share", now_unix_ns);
        };
        Some(after_share_id)
    };
    match envelopes
        .list_key_envelopes(&identity.device, after_share_id)
        .await
    {
        Ok(found) => reply_with(
            request,
            Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
                envelopes: found.envelopes.into_iter().map(sealed).collect(),
                next_after_share_id: found
                    .next_after_share_id
                    .map_or_else(Vec::new, |share_id| share_id.to_vec()),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

fn sealed(record: KeyEnvelopeRecord) -> KeyEnvelope {
    KeyEnvelope {
        share_id: record.share_id.to_vec(),
        ephemeral_public_key: record.ephemeral_public_key.to_vec(),
        ciphertext: record.ciphertext,
    }
}

fn unauthenticated(request: &Envelope, now_unix_ns: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        "authenticate before using key envelopes".to_owned(),
        now_unix_ns,
    )
}

fn malformed(request: &Envelope, message: &str, now_unix_ns: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::InvalidMessage,
        request.message_id.clone(),
        message.to_owned(),
        now_unix_ns,
    )
}

/// Maps an envelope failure onto the wire, keeping storage detail off it.
fn rejection(request: &Envelope, error: &EnvelopeError, now_unix_ns: u64) -> Envelope {
    let code = match error {
        EnvelopeError::InvalidEphemeralKeyLength { .. }
        | EnvelopeError::CiphertextTooLarge { .. } => ProtocolErrorCode::InvalidMessage,
        EnvelopeError::UnknownRecipient
        | EnvelopeError::NotYourDevice
        | EnvelopeError::RecipientRevoked => ProtocolErrorCode::Unauthorized,
        EnvelopeError::Repository(_) => ProtocolErrorCode::Internal,
    };
    let message = match error {
        EnvelopeError::Repository(_) => "the identity store is unavailable".to_owned(),
        other => other.to_string(),
    };
    protocol_error(code, request.message_id.clone(), message, now_unix_ns)
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::v1::{Ping, ProtocolError};
    use portalis_nexus_protocol::{
        ENCRYPTION_KEY_BYTES, MAX_KEY_ENVELOPES_PER_PAGE, new_message_id,
    };
    use portalis_nexus_server_core::{
        DeviceRecord, Identity, IdentityRepository, ProtocolPolicy, RepositoryError, UserRecord,
    };

    use super::*;
    use crate::state::AppState;

    const NOW: u64 = 1_700_000_000_000_000_000;
    const ADA: [u8; 16] = [1; 16];
    const SHARE: [u8; SHARE_ID_BYTES] = [3; SHARE_ID_BYTES];

    fn request() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        }
    }

    fn refusal(reply: &Envelope) -> Option<(ProtocolErrorCode, String)> {
        match &reply.payload {
            Some(Payload::ProtocolError(ProtocolError { code, message, .. })) => Some((
                ProtocolErrorCode::try_from(*code).unwrap_or(ProtocolErrorCode::Unspecified),
                message.clone(),
            )),
            _ => None,
        }
    }

    fn listed(reply: &Envelope) -> Option<ListKeyEnvelopesResponse> {
        match &reply.payload {
            Some(Payload::ListKeyEnvelopesResponse(response)) => Some(response.clone()),
            _ => None,
        }
    }

    /// A connection already bound to a registered user and its own device.
    async fn signed_in(state: &AppState, seed: u8) -> Session {
        let user = UserRecord {
            user_id: ADA,
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        };
        let device = DeviceRecord {
            device_id: [seed; 32],
            user_id: ADA,
            public_key: [seed; 32],
            encryption_public_key: [seed; 32],
            created_at_unix_ns: NOW,
            last_authenticated_at_unix_ns: Some(NOW),
            revoked_at_unix_ns: None,
        };
        state
            .store()
            .insert_registration(user.clone(), device.clone())
            .await
            .expect("seeded");

        let policy = ProtocolPolicy::new(1, 1).expect("range");
        let mut session = Session::new(&crate::messages::hello_payload(&policy, NOW));
        session.bind(Identity { user, device });
        session
    }

    fn put_request(share_id: Vec<u8>, recipient_device_id: Vec<u8>) -> PutKeyEnvelope {
        PutKeyEnvelope {
            share_id,
            recipient_device_id,
            ephemeral_public_key: vec![9; ENCRYPTION_KEY_BYTES],
            ciphertext: b"sealed".to_vec(),
        }
    }

    /// Identifiers arrive as bare bytes, so a wrong length is a client
    /// mistake to refuse rather than something to truncate or pad into a
    /// different share or device.
    #[tokio::test]
    async fn identifiers_of_the_wrong_length_are_refused() {
        let state = AppState::default();
        let session = signed_in(&state, 1).await;

        for (put, expected) in [
            (
                put_request(vec![3; SHARE_ID_BYTES - 1], vec![1; 32]),
                "share_id must name a share",
            ),
            (
                put_request(SHARE.to_vec(), vec![1; 31]),
                "recipient_device_id must name a device",
            ),
        ] {
            let reply = super::put(&session, state.envelopes(), &request(), &put, NOW).await;

            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, expected);
        }
    }

    #[tokio::test]
    async fn a_cursor_of_the_wrong_length_is_refused() {
        let state = AppState::default();
        let session = signed_in(&state, 1).await;

        let reply = list(
            &session,
            state.envelopes(),
            &request(),
            &ListKeyEnvelopesRequest {
                after_share_id: vec![3; SHARE_ID_BYTES + 1],
            },
            NOW,
        )
        .await;

        let (code, message) = refusal(&reply).expect("a refusal");
        assert_eq!(code, ProtocolErrorCode::InvalidMessage);
        assert_eq!(message, "after_share_id must name a share");
    }

    /// An empty cursor means the first page; a well-formed one resumes after
    /// the share it names.
    #[tokio::test]
    async fn a_well_formed_cursor_resumes_the_listing() {
        let state = AppState::default();
        let session = signed_in(&state, 1).await;
        super::put(
            &session,
            state.envelopes(),
            &request(),
            &put_request(SHARE.to_vec(), vec![1; 32]),
            NOW,
        )
        .await;

        let first = list(
            &session,
            state.envelopes(),
            &request(),
            &ListKeyEnvelopesRequest {
                after_share_id: Vec::new(),
            },
            NOW,
        )
        .await;
        let found = listed(&first).expect("a listing");
        assert_eq!(found.envelopes.len(), 1);
        assert_eq!(found.envelopes[0].share_id, SHARE.to_vec());
        assert_eq!(found.envelopes[0].ciphertext, b"sealed".to_vec());
        assert!(found.next_after_share_id.is_empty(), "nothing follows");

        // Resuming after the only share leaves nothing to return.
        let resumed = list(
            &session,
            state.envelopes(),
            &request(),
            &ListKeyEnvelopesRequest {
                after_share_id: SHARE.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(listed(&resumed).expect("a listing").envelopes, Vec::new());
        assert!(listed(&request()).is_none(), "a request is not a listing");
    }

    /// A page that does not carry everything must tell the caller where to
    /// resume, or a device would silently stop at the first page and never
    /// learn about the shares beyond it.
    #[tokio::test]
    async fn a_full_page_carries_the_cursor_onto_the_wire() {
        let state = AppState::default();
        let session = signed_in(&state, 1).await;
        for index in 0..=MAX_KEY_ENVELOPES_PER_PAGE {
            let mut share_id = SHARE;
            share_id[SHARE_ID_BYTES - 2..].copy_from_slice(
                &u16::try_from(index)
                    .expect("the page limit fits")
                    .to_be_bytes(),
            );
            super::put(
                &session,
                state.envelopes(),
                &request(),
                &put_request(share_id.to_vec(), vec![1; 32]),
                NOW,
            )
            .await;
        }

        let reply = list(
            &session,
            state.envelopes(),
            &request(),
            &ListKeyEnvelopesRequest {
                after_share_id: Vec::new(),
            },
            NOW,
        )
        .await;

        let found = listed(&reply).expect("a listing");
        assert_eq!(found.envelopes.len(), MAX_KEY_ENVELOPES_PER_PAGE);
        assert_eq!(
            found.next_after_share_id,
            found
                .envelopes
                .last()
                .expect("a full page")
                .share_id
                .clone(),
            "the cursor is the last share on the page"
        );
    }

    #[tokio::test]
    async fn a_store_outage_is_reported_by_both_commands() {
        let state = AppState::default();
        let session = signed_in(&state, 1).await;
        state.store().set_unavailable(true);

        let replies = [
            super::put(
                &session,
                state.envelopes(),
                &request(),
                &put_request(SHARE.to_vec(), vec![1; 32]),
                NOW,
            )
            .await,
            list(
                &session,
                state.envelopes(),
                &request(),
                &ListKeyEnvelopesRequest {
                    after_share_id: Vec::new(),
                },
                NOW,
            )
            .await,
        ];

        for reply in &replies {
            let (code, message) = refusal(reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::Internal);
            assert_eq!(message, "the identity store is unavailable");
        }
    }

    #[test]
    fn every_envelope_failure_maps_onto_a_typed_refusal() {
        let request = request();

        for (error, expected) in [
            (
                EnvelopeError::InvalidEphemeralKeyLength { actual: 3 },
                ProtocolErrorCode::InvalidMessage,
            ),
            (
                EnvelopeError::CiphertextTooLarge { actual: 99_999 },
                ProtocolErrorCode::InvalidMessage,
            ),
            (
                EnvelopeError::UnknownRecipient,
                ProtocolErrorCode::Unauthorized,
            ),
            (
                EnvelopeError::NotYourDevice,
                ProtocolErrorCode::Unauthorized,
            ),
            (
                EnvelopeError::RecipientRevoked,
                ProtocolErrorCode::Unauthorized,
            ),
        ] {
            let reply = rejection(&request, &error, NOW);

            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, expected, "for {error}");
            assert_eq!(message, error.to_string(), "the reason reaches the caller");
            assert_eq!(reply.correlation_id, request.message_id);
        }
    }

    #[test]
    fn storage_detail_never_reaches_the_wire() {
        let outage = EnvelopeError::Repository(RepositoryError::Unavailable(
            "connection refused to db-1.internal".to_owned(),
        ));

        let reply = rejection(&request(), &outage, NOW);

        let (code, message) = refusal(&reply).expect("a refusal");
        assert_eq!(code, ProtocolErrorCode::Internal);
        assert_eq!(message, "the identity store is unavailable");
        assert!(!message.contains("db-1.internal"));
        assert!(refusal(&request()).is_none(), "a request is not a refusal");
    }

    #[tokio::test]
    async fn key_envelopes_require_authentication() {
        let state = AppState::default();
        let session = Session::new(&crate::messages::hello_payload(
            &ProtocolPolicy::new(1, 1).expect("range"),
            NOW,
        ));

        let replies = [
            super::put(
                &session,
                state.envelopes(),
                &request(),
                &put_request(SHARE.to_vec(), vec![1; 32]),
                NOW,
            )
            .await,
            list(
                &session,
                state.envelopes(),
                &request(),
                &ListKeyEnvelopesRequest {
                    after_share_id: Vec::new(),
                },
                NOW,
            )
            .await,
        ];

        for reply in &replies {
            assert_eq!(
                refusal(reply).map(|(code, _)| code),
                Some(ProtocolErrorCode::Unauthenticated)
            );
        }
    }
}
