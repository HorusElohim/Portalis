//! Private encrypted-share publication, authorization, and live handoff.

use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, FetchShareRequest, FetchShareResponse, GrantShareAccess, ListSharesResponse,
    ProtocolErrorCode, PublishShare, RevokeShareAccess, ShareAccessGranted, ShareAccessRevoked,
    ShareEvent, ShareHandoff, SharePublished, ShareRecipientDevice, ShareSnapshot,
};
use portalis_nexus_protocol::{
    INFO_HASH_V1_BYTES, MAX_SHARE_HANDOFF_BYTES, SHARE_ID_BYTES, SNAPSHOT_ID_BYTES, USER_ID_BYTES,
};
use portalis_nexus_server_core::{
    DeviceId, IdentityRepository, Publication, ShareCommandError, ShareRecord, ShareRepository,
};

use crate::messages::{binary_frame, protocol_error, reply_with};
use crate::session::Session;
use crate::state::AppState;

type PublicationIds = (
    [u8; SHARE_ID_BYTES],
    [u8; SNAPSHOT_ID_BYTES],
    Option<[u8; SNAPSHOT_ID_BYTES]>,
);

pub(crate) async fn publish(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    command: &PublishShare,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Some((share_id, snapshot_id, prior_snapshot_id)) = publication_ids(command) else {
        return malformed(request, "invalid share or snapshot identifier", now_unix_ns);
    };
    match state
        .shares()
        .publish(Publication {
            share_id,
            publisher: identity.user.user_id,
            revision: command.revision,
            prior_snapshot_id,
            snapshot_id,
            capsule: &command.capsule,
            capsule_signature: &command.capsule_signature,
        })
        .await
    {
        Ok(share) => {
            announce(state, &share, now_unix_ns).await;
            reply_with(
                request,
                Payload::SharePublished(SharePublished {
                    share: Some(snapshot(&share)),
                }),
                now_unix_ns,
            )
        }
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

pub(crate) async fn list(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    match state.shares().list(identity.user.user_id).await {
        Ok(shares) => reply_with(
            request,
            Payload::ListSharesResponse(ListSharesResponse {
                shares: shares.iter().map(snapshot).collect(),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

pub(crate) async fn fetch(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    fetch: &FetchShareRequest,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(fetch.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ns);
    };
    match state.shares().fetch(identity.user.user_id, share_id).await {
        Ok(share) => reply_with(
            request,
            Payload::FetchShareResponse(FetchShareResponse {
                share: Some(snapshot(&share)),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

pub(crate) async fn grant(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    grant: &GrantShareAccess,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(grant.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ns);
    };
    let Ok(member) = <[u8; USER_ID_BYTES]>::try_from(grant.member_user_id.as_slice()) else {
        return malformed(request, "member_user_id must name a user", now_unix_ns);
    };
    match state
        .shares()
        .grant(identity.user.user_id, share_id, member)
        .await
    {
        Ok(()) => match state.store().list_devices(member).await {
            Ok(devices) => reply_with(
                request,
                Payload::ShareAccessGranted(ShareAccessGranted {
                    share_id: share_id.to_vec(),
                    member_user_id: member.to_vec(),
                    recipient_devices: devices
                        .into_iter()
                        .take(16)
                        .map(|device| ShareRecipientDevice {
                            device_id: device.device_id.to_vec(),
                            encryption_public_key: device.encryption_public_key.to_vec(),
                        })
                        .collect(),
                }),
                now_unix_ns,
            ),
            Err(_) => protocol_error(
                ProtocolErrorCode::Internal,
                request.message_id.clone(),
                "the identity store is unavailable".to_owned(),
                now_unix_ns,
            ),
        },
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

pub(crate) async fn revoke(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    revoke: &RevokeShareAccess,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(revoke.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ns);
    };
    let Ok(member) = <[u8; USER_ID_BYTES]>::try_from(revoke.member_user_id.as_slice()) else {
        return malformed(request, "member_user_id must name a user", now_unix_ns);
    };
    match state
        .shares()
        .revoke(identity.user.user_id, share_id, member)
        .await
    {
        Ok(()) => reply_with(
            request,
            Payload::ShareAccessRevoked(ShareAccessRevoked {
                share_id: share_id.to_vec(),
                member_user_id: member.to_vec(),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

pub(crate) async fn handoff(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    handoff: &ShareHandoff,
    now_unix_ns: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(handoff.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ns);
    };
    let Ok(recipient) = DeviceId::try_from(handoff.recipient_device_id.as_slice()) else {
        return malformed(
            request,
            "recipient_device_id must name a device",
            now_unix_ns,
        );
    };
    if handoff.ciphertext.len() > MAX_SHARE_HANDOFF_BYTES {
        return malformed(request, "encrypted handoff is too large", now_unix_ns);
    }
    if handoff.info_hash.len() != INFO_HASH_V1_BYTES {
        return malformed(request, "info_hash must be a v1 info hash", now_unix_ns);
    }
    let sender_allowed = state
        .store()
        .has_share_access(share_id, identity.user.user_id)
        .await;
    let recipient_device = state.store().find_device(recipient).await;
    let (Ok(true), Ok(Some(recipient_device))) = (sender_allowed, recipient_device) else {
        return private(request, now_unix_ns);
    };
    if !state
        .store()
        .has_share_access(share_id, recipient_device.user_id)
        .await
        .unwrap_or(false)
    {
        return private(request, now_unix_ns);
    }

    let connections = state.presence().connections_of_device(recipient);
    if connections.is_empty() {
        return unavailable(request, now_unix_ns);
    }
    let event = Envelope {
        message_id: portalis_nexus_protocol::new_message_id(),
        correlation_id: Vec::new(),
        timestamp_unix_ns: now_unix_ns,
        payload: Some(Payload::ShareHandoff(handoff.clone())),
    };
    for connection in connections {
        state.connections().send(connection, binary_frame(&event));
    }
    reply_with(
        request,
        Payload::ShareHandoff(ShareHandoff {
            share_id: share_id.to_vec(),
            recipient_device_id: Vec::new(),
            ciphertext: Vec::new(),
            info_hash: handoff.info_hash.clone(),
        }),
        now_unix_ns,
    )
}

async fn announce(state: &AppState, share: &ShareRecord, now_unix_ns: u64) {
    let event = Envelope {
        message_id: portalis_nexus_protocol::new_message_id(),
        correlation_id: Vec::new(),
        timestamp_unix_ns: now_unix_ns,
        payload: Some(Payload::ShareEvent(ShareEvent {
            share: Some(snapshot(share)),
        })),
    };
    if let Ok(members) = state.shares().members(share.share_id).await {
        let frame = binary_frame(&event);
        for member in members {
            for connection in state.presence().connections_of(member) {
                state.connections().send(connection, frame.clone());
            }
        }
    }
}

fn publication_ids(command: &PublishShare) -> Option<PublicationIds> {
    let share_id = command.share_id.as_slice().try_into().ok()?;
    let snapshot_id = command.snapshot_id.as_slice().try_into().ok()?;
    let prior = if command.prior_snapshot_id.is_empty() {
        None
    } else {
        Some(command.prior_snapshot_id.as_slice().try_into().ok()?)
    };
    Some((share_id, snapshot_id, prior))
}

fn snapshot(share: &ShareRecord) -> ShareSnapshot {
    ShareSnapshot {
        share_id: share.share_id.to_vec(),
        owner_user_id: share.owner.to_vec(),
        revision: share.revision,
        snapshot_id: share.snapshot_id.to_vec(),
        capsule: share.capsule.clone(),
        capsule_signature: share.capsule_signature.clone(),
        created_at_unix_ns: share.updated_at_unix_ns,
    }
}

fn unauthenticated(request: &Envelope, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        "authenticate before using shares".to_owned(),
        now,
    )
}

fn malformed(request: &Envelope, message: &str, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::InvalidMessage,
        request.message_id.clone(),
        message.to_owned(),
        now,
    )
}

fn private(request: &Envelope, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::NotFound,
        request.message_id.clone(),
        "that share was not found".to_owned(),
        now,
    )
}

fn unavailable(request: &Envelope, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unavailable,
        request.message_id.clone(),
        "the recipient device is offline".to_owned(),
        now,
    )
}

fn rejection(request: &Envelope, error: &ShareCommandError, now: u64) -> Envelope {
    let code = match error {
        ShareCommandError::CapsuleTooLarge { .. }
        | ShareCommandError::InvalidSignatureLength { .. }
        | ShareCommandError::Publication(_) => ProtocolErrorCode::InvalidMessage,
        ShareCommandError::NotFound => ProtocolErrorCode::NotFound,
        ShareCommandError::NotTheOwner
        | ShareCommandError::UnknownMember
        | ShareCommandError::OwnerCannotBeRemoved => ProtocolErrorCode::Unauthorized,
        ShareCommandError::Repository(_) => ProtocolErrorCode::Internal,
    };
    let message = if matches!(error, ShareCommandError::Repository(_)) {
        "the share store is unavailable".to_owned()
    } else {
        error.to_string()
    };
    protocol_error(code, request.message_id.clone(), message, now)
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::v1::{ListSharesRequest, Ping, ProtocolError};
    use portalis_nexus_protocol::{MAX_SHARE_CAPSULE_BYTES, SIGNATURE_BYTES, new_message_id};
    use portalis_nexus_server_core::{
        DeviceRecord, Identity, ProtocolPolicy, ShareRepository, UserRecord,
    };
    use tokio::sync::mpsc;

    use super::*;

    const NOW: u64 = 1_700_000_000_000_000_000;
    const ADA: [u8; USER_ID_BYTES] = [1; USER_ID_BYTES];
    const GRACE: [u8; USER_ID_BYTES] = [2; USER_ID_BYTES];
    const SHARE: [u8; SHARE_ID_BYTES] = [3; SHARE_ID_BYTES];
    const SNAPSHOT: [u8; SNAPSHOT_ID_BYTES] = [4; SNAPSHOT_ID_BYTES];

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

    fn revocation(reply: &Envelope) -> Option<ShareAccessRevoked> {
        match &reply.payload {
            Some(Payload::ShareAccessRevoked(revoked)) => Some(revoked.clone()),
            _ => None,
        }
    }

    fn published(reply: &Envelope) -> Option<ShareSnapshot> {
        match &reply.payload {
            Some(Payload::SharePublished(published)) => published.share.clone(),
            _ => None,
        }
    }

    fn listed(reply: &Envelope) -> Option<Vec<ShareSnapshot>> {
        match &reply.payload {
            Some(Payload::ListSharesResponse(response)) => Some(response.shares.clone()),
            _ => None,
        }
    }

    fn fetched(reply: &Envelope) -> Option<ShareSnapshot> {
        match &reply.payload {
            Some(Payload::FetchShareResponse(response)) => response.share.clone(),
            _ => None,
        }
    }

    fn handed_off(reply: &Envelope) -> Option<ShareHandoff> {
        match &reply.payload {
            Some(Payload::ShareHandoff(handoff)) => Some(handoff.clone()),
            _ => None,
        }
    }

    fn announced(reply: &Envelope) -> Option<ShareSnapshot> {
        match &reply.payload {
            Some(Payload::ShareEvent(event)) => event.share.clone(),
            _ => None,
        }
    }

    /// The envelope inside one queued outbound frame.
    fn pushed(message: &axum::extract::ws::Message) -> Option<Envelope> {
        match message {
            axum::extract::ws::Message::Binary(bytes) => {
                portalis_nexus_protocol::decode_frame(bytes).ok()
            }
            _ => None,
        }
    }

    /// A connection bound to a registered user and one of its devices.
    async fn signed_in(state: &AppState, user_id: [u8; USER_ID_BYTES], seed: u8) -> Session {
        let user = UserRecord {
            user_id,
            username: format!("user{seed}"),
            normalized_username: format!("user{seed}"),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        };
        let device = DeviceRecord {
            device_id: [seed; 32],
            user_id,
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

    fn publication(revision: u64, capsule: &[u8]) -> PublishShare {
        PublishShare {
            share_id: SHARE.to_vec(),
            revision,
            prior_snapshot_id: Vec::new(),
            snapshot_id: SNAPSHOT.to_vec(),
            capsule: capsule.to_vec(),
            capsule_signature: vec![8; SIGNATURE_BYTES],
        }
    }

    /// Every share command needs an identity: an anonymous connection cannot
    /// be authorized against anything, so none of them may run.
    #[tokio::test]
    async fn every_command_refuses_an_unauthenticated_connection() {
        let state = AppState::default();
        let policy = ProtocolPolicy::new(1, 1).expect("range");
        let session = Session::new(&crate::messages::hello_payload(&policy, NOW));

        let replies = [
            publish(&session, &state, &request(), &publication(1, b"c"), NOW).await,
            list(&session, &state, &request(), NOW).await,
            fetch(
                &session,
                &state,
                &request(),
                &FetchShareRequest {
                    share_id: SHARE.to_vec(),
                },
                NOW,
            )
            .await,
            grant(
                &session,
                &state,
                &request(),
                &GrantShareAccess {
                    share_id: SHARE.to_vec(),
                    member_user_id: GRACE.to_vec(),
                },
                NOW,
            )
            .await,
            handoff(
                &session,
                &state,
                &request(),
                &ShareHandoff {
                    share_id: SHARE.to_vec(),
                    recipient_device_id: vec![2; 32],
                    ciphertext: b"sealed".to_vec(),
                    info_hash: vec![7; 20],
                },
                NOW,
            )
            .await,
        ];

        for reply in &replies {
            let (code, message) = refusal(reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::Unauthenticated);
            assert_eq!(message, "authenticate before using shares");
        }
    }

    /// Identifiers arrive as bare bytes, so a wrong length is a client mistake
    /// to refuse rather than something to pad into a different share.
    #[tokio::test]
    async fn identifiers_of_the_wrong_length_are_refused() {
        let state = AppState::default();
        let session = signed_in(&state, ADA, 1).await;

        let short_share = PublishShare {
            share_id: vec![3; SHARE_ID_BYTES - 1],
            ..publication(1, b"c")
        };
        let short_snapshot = PublishShare {
            snapshot_id: vec![4; SNAPSHOT_ID_BYTES - 1],
            ..publication(1, b"c")
        };
        let short_prior = PublishShare {
            prior_snapshot_id: vec![4; SNAPSHOT_ID_BYTES - 1],
            ..publication(2, b"c")
        };
        for command in [short_share, short_snapshot, short_prior] {
            let reply = publish(&session, &state, &request(), &command, NOW).await;
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, "invalid share or snapshot identifier");
        }

        let reply = fetch(
            &session,
            &state,
            &request(),
            &FetchShareRequest {
                share_id: vec![3; SHARE_ID_BYTES + 1],
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::InvalidMessage,
                "share_id must name a share".to_owned()
            )
        );

        for (command, expected) in [
            (
                GrantShareAccess {
                    share_id: vec![3; SHARE_ID_BYTES - 1],
                    member_user_id: GRACE.to_vec(),
                },
                "share_id must name a share",
            ),
            (
                GrantShareAccess {
                    share_id: SHARE.to_vec(),
                    member_user_id: vec![2; USER_ID_BYTES - 1],
                },
                "member_user_id must name a user",
            ),
        ] {
            let reply = grant(&session, &state, &request(), &command, NOW).await;
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, expected);
        }
    }

    /// Publishing writes the head and its immutable history row, answers with
    /// the snapshot the share now points at, and lists it back to its owner.
    #[tokio::test]
    async fn publishing_answers_with_the_share_and_keeps_its_history() {
        let state = AppState::default();
        let session = signed_in(&state, ADA, 1).await;

        let reply = publish(
            &session,
            &state,
            &request(),
            &publication(1, b"encrypted"),
            NOW,
        )
        .await;

        let share = published(&reply).expect("a publication");
        assert_eq!(share.share_id, SHARE.to_vec());
        assert_eq!(share.owner_user_id, ADA.to_vec());
        assert_eq!(share.revision, 1);
        assert_eq!(share.capsule, b"encrypted".to_vec());
        // Read through the Arc-wrapped store the server actually runs on.
        assert!(
            state
                .store()
                .find_snapshot(SHARE, 1)
                .await
                .expect("read")
                .is_some(),
            "revision one is kept as history, not only as the head"
        );

        let shares = listed(&list(&session, &state, &request(), NOW).await).expect("a listing");
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].revision, 1);
        let _ = ListSharesRequest {};

        // Each matcher recognises only its own answer, so a test that reads
        // the wrong one fails rather than quietly matching nothing.
        let other = request();
        assert!(published(&other).is_none());
        assert!(listed(&other).is_none());
        assert!(fetched(&other).is_none());
        assert!(handed_off(&other).is_none());
        assert!(announced(&other).is_none());
        assert!(revocation(&other).is_none());
        assert!(refusal(&other).is_none());
        assert!(pushed(&axum::extract::ws::Message::Text("not a frame".into())).is_none());
    }

    /// A share is private: a stranger asking for one by identifier learns
    /// nothing, and is told the same thing whether or not it exists.
    #[tokio::test]
    async fn a_stranger_cannot_fetch_or_probe_a_private_share() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let stranger = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;

        let refused = fetch(
            &stranger,
            &state,
            &request(),
            &FetchShareRequest {
                share_id: SHARE.to_vec(),
            },
            NOW,
        )
        .await;
        let missing = fetch(
            &stranger,
            &state,
            &request(),
            &FetchShareRequest {
                share_id: [9; SHARE_ID_BYTES].to_vec(),
            },
            NOW,
        )
        .await;

        assert_eq!(refusal(&refused), refusal(&missing));
        assert_eq!(
            refusal(&refused).expect("a refusal").0,
            ProtocolErrorCode::NotFound
        );

        // Granted access, the same fetch succeeds.
        let granted = grant(
            &owner,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;
        let devices = granted_devices(&granted).expect("a grant names the devices");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, vec![2; 32]);
        assert_eq!(devices[0].encryption_public_key, vec![2; 32]);

        let allowed = fetch(
            &stranger,
            &state,
            &request(),
            &FetchShareRequest {
                share_id: SHARE.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            fetched(&allowed).expect("the share").share_id,
            SHARE.to_vec()
        );
    }

    /// Members hear about a new revision without asking, because a client that
    /// only learned on reconnect would show a stale share indefinitely.
    #[tokio::test]
    async fn members_are_told_when_a_share_moves() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let member = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"one"), NOW).await;
        grant(
            &owner,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;

        let (outbound, mut inbox) = mpsc::channel(4);
        state
            .connections()
            .register(member.connection_id(), outbound);
        assert!(
            state
                .presence()
                .arrive_for_device(
                    GRACE,
                    member
                        .identity()
                        .expect("member is signed in")
                        .device
                        .device_id,
                    member.connection_id(),
                )
                .is_some(),
            "the member is online"
        );

        let next = PublishShare {
            prior_snapshot_id: SNAPSHOT.to_vec(),
            snapshot_id: [5; SNAPSHOT_ID_BYTES].to_vec(),
            ..publication(2, b"two")
        };
        publish(&owner, &state, &request(), &next, NOW).await;

        let event = pushed(&inbox.try_recv().expect("the member was told")).expect("an envelope");
        assert_eq!(announced(&event).expect("a share event").revision, 2);

        // Telling members is best-effort. A store that fails while listing
        // them leaves the publication standing rather than undoing a durable
        // write over a notification nobody was waiting on; clients refresh on
        // reconnect.
        state.store().set_unavailable(true);
        super::announce(
            &state,
            &ShareRecord {
                share_id: SHARE,
                owner: ADA,
                revision: 3,
                snapshot_id: [6; SNAPSHOT_ID_BYTES],
                capsule: b"three".to_vec(),
                capsule_signature: vec![8; SIGNATURE_BYTES],
                created_at_unix_ns: NOW,
                updated_at_unix_ns: NOW,
            },
            NOW,
        )
        .await;
        assert!(
            inbox.try_recv().is_err(),
            "nobody could be looked up, so nobody was told"
        );
    }

    /// A live handoff only moves between two devices that may both already
    /// read the share, so it cannot be used to reach a stranger.
    #[tokio::test]
    async fn a_handoff_reaches_only_devices_that_may_read_the_share() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let member = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;

        let sealed = ShareHandoff {
            share_id: SHARE.to_vec(),
            recipient_device_id: vec![2; 32],
            ciphertext: b"magnet".to_vec(),
            info_hash: vec![7; 20],
        };

        // Not yet a member: refused, and told only that it was not found.
        let refused = handoff(&owner, &state, &request(), &sealed, NOW).await;
        assert_eq!(
            refusal(&refused).expect("a refusal").0,
            ProtocolErrorCode::NotFound
        );

        grant(
            &owner,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;
        let (outbound, mut inbox) = mpsc::channel(4);
        state
            .connections()
            .register(member.connection_id(), outbound);
        assert!(
            state
                .presence()
                .arrive_for_device(
                    GRACE,
                    member
                        .identity()
                        .expect("member is signed in")
                        .device
                        .device_id,
                    member.connection_id(),
                )
                .is_some(),
            "the member is online"
        );

        let reply = handoff(&owner, &state, &request(), &sealed, NOW).await;

        let acknowledged = handed_off(&reply).expect("an acknowledgement");
        assert_eq!(acknowledged.share_id, SHARE.to_vec());
        assert!(
            acknowledged.ciphertext.is_empty(),
            "the acknowledgement does not echo the sealed payload back"
        );
        let forwarded =
            pushed(&inbox.try_recv().expect("the recipient was reached")).expect("an envelope");
        assert_eq!(
            handed_off(&forwarded).expect("a handoff").ciphertext,
            b"magnet".to_vec()
        );
    }

    /// An unknown recipient device and an oversized payload are both refused
    /// before anything is forwarded.
    #[tokio::test]
    async fn a_handoff_is_bounded_and_needs_a_real_recipient() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;

        let oversized = ShareHandoff {
            share_id: SHARE.to_vec(),
            recipient_device_id: vec![1; 32],
            ciphertext: vec![0; MAX_SHARE_HANDOFF_BYTES + 1],
            info_hash: vec![7; 20],
        };
        let reply = handoff(&owner, &state, &request(), &oversized, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::InvalidMessage,
                "encrypted handoff is too large".to_owned()
            )
        );

        for (command, expected) in [
            (
                ShareHandoff {
                    share_id: vec![3; SHARE_ID_BYTES - 1],
                    recipient_device_id: vec![1; 32],
                    ciphertext: b"m".to_vec(),
                    info_hash: vec![7; 20],
                },
                "share_id must name a share",
            ),
            (
                ShareHandoff {
                    share_id: SHARE.to_vec(),
                    recipient_device_id: vec![1; 31],
                    ciphertext: b"m".to_vec(),
                    info_hash: vec![7; 20],
                },
                "recipient_device_id must name a device",
            ),
            (
                ShareHandoff {
                    share_id: SHARE.to_vec(),
                    recipient_device_id: vec![1; 32],
                    ciphertext: b"m".to_vec(),
                    info_hash: vec![7; 19],
                },
                "info_hash must be a v1 info hash",
            ),
        ] {
            let reply = handoff(&owner, &state, &request(), &command, NOW).await;
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, expected);
        }

        // A device nobody enrolled is indistinguishable from a private share.
        let unknown = ShareHandoff {
            share_id: SHARE.to_vec(),
            recipient_device_id: vec![9; 32],
            ciphertext: b"m".to_vec(),
            info_hash: vec![7; 20],
        };
        let reply = handoff(&owner, &state, &request(), &unknown, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::NotFound
        );
    }

    /// Granting and revoking are inverses on the wire, and a revoked member
    /// stops being able to reach the share at all.
    #[tokio::test]
    async fn revoking_takes_a_members_access_back() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let member = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;
        let membership = GrantShareAccess {
            share_id: SHARE.to_vec(),
            member_user_id: GRACE.to_vec(),
        };
        grant(&owner, &state, &request(), &membership, NOW).await;

        let reply = revoke(
            &owner,
            &state,
            &request(),
            &RevokeShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;

        let revoked = revocation(&reply).expect("a revocation");
        assert_eq!(revoked.share_id, SHARE.to_vec());
        assert_eq!(revoked.member_user_id, GRACE.to_vec());

        let refused = fetch(
            &member,
            &state,
            &request(),
            &FetchShareRequest {
                share_id: SHARE.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&refused).expect("a refusal").0,
            ProtocolErrorCode::NotFound
        );
        assert_eq!(
            listed(&list(&member, &state, &request(), NOW).await).expect("a listing"),
            Vec::new()
        );
    }

    /// The commands a client would get wrong: an anonymous connection, bad
    /// identifiers, someone else's share, and the owner themselves.
    #[tokio::test]
    async fn revoking_refuses_everything_it_should() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let stranger = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;

        let policy = ProtocolPolicy::new(1, 1).expect("range");
        let anonymous = Session::new(&crate::messages::hello_payload(&policy, NOW));
        let well_formed = RevokeShareAccess {
            share_id: SHARE.to_vec(),
            member_user_id: GRACE.to_vec(),
        };

        let reply = revoke(&anonymous, &state, &request(), &well_formed, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::Unauthenticated,
                "authenticate before using shares".to_owned()
            )
        );

        for (command, expected) in [
            (
                RevokeShareAccess {
                    share_id: vec![3; SHARE_ID_BYTES - 1],
                    member_user_id: GRACE.to_vec(),
                },
                "share_id must name a share",
            ),
            (
                RevokeShareAccess {
                    share_id: SHARE.to_vec(),
                    member_user_id: vec![2; USER_ID_BYTES + 1],
                },
                "member_user_id must name a user",
            ),
        ] {
            let reply = revoke(&owner, &state, &request(), &command, NOW).await;
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, expected);
        }

        // A member cannot revoke, and nobody can remove the owner.
        let reply = revoke(&stranger, &state, &request(), &well_formed, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::Unauthorized
        );
        let reply = revoke(
            &owner,
            &state,
            &request(),
            &RevokeShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: ADA.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::Unauthorized,
                "a share's owner cannot be removed from it".to_owned()
            )
        );

        // Storage detail stays off the wire here too.
        state.store().set_unavailable(true);
        let reply = revoke(&owner, &state, &request(), &well_formed, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::Internal,
                "the share store is unavailable".to_owned()
            )
        );
    }

    /// A publisher whose acknowledgement was lost retries the same bytes.
    /// That has to succeed, or a dropped reply would strand a device unable
    /// either to move forward or to repeat itself.
    #[tokio::test]
    async fn republishing_the_same_bytes_succeeds_without_moving_the_share() {
        let state = AppState::default();
        let session = signed_in(&state, ADA, 1).await;
        let command = publication(1, b"encrypted");

        let first = publish(&session, &state, &request(), &command, NOW).await;
        let again = publish(&session, &state, &request(), &command, NOW).await;

        assert_eq!(published(&first), published(&again));
        assert_eq!(published(&again).expect("a publication").revision, 1);

        // Different bytes for a revision already published are refused: a
        // revision is immutable once written.
        let rewritten = publish(
            &session,
            &state,
            &request(),
            &publication(1, b"rewritten"),
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&rewritten).expect("a refusal").0,
            ProtocolErrorCode::InvalidMessage
        );
    }

    /// A publication has to name the snapshot the share is actually on, so
    /// one built against a revision another device already replaced is
    /// refused rather than silently overwriting it.
    #[tokio::test]
    async fn a_publication_must_follow_the_snapshot_the_share_is_on() {
        let state = AppState::default();
        let session = signed_in(&state, ADA, 1).await;

        // Nothing to follow yet, so naming a prior snapshot is a publisher
        // working from a share this server has no record of.
        let invented_history = PublishShare {
            prior_snapshot_id: SNAPSHOT.to_vec(),
            ..publication(1, b"one")
        };
        let reply = publish(&session, &state, &request(), &invented_history, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::InvalidMessage
        );

        publish(&session, &state, &request(), &publication(1, b"one"), NOW).await;

        let built_on_the_wrong_snapshot = PublishShare {
            prior_snapshot_id: [9; SNAPSHOT_ID_BYTES].to_vec(),
            snapshot_id: [5; SNAPSHOT_ID_BYTES].to_vec(),
            ..publication(2, b"two")
        };
        let reply = publish(
            &session,
            &state,
            &request(),
            &built_on_the_wrong_snapshot,
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::InvalidMessage
        );

        // A peer may seed and fetch a share without being able to move it.
        let stranger = signed_in(&state, GRACE, 2).await;
        let taking_it_over = PublishShare {
            prior_snapshot_id: SNAPSHOT.to_vec(),
            snapshot_id: [5; SNAPSHOT_ID_BYTES].to_vec(),
            ..publication(2, b"mine now")
        };
        let reply = publish(&stranger, &state, &request(), &taking_it_over, NOW).await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::InvalidMessage
        );
        assert_eq!(
            published(
                &fetch(
                    &session,
                    &state,
                    &request(),
                    &FetchShareRequest {
                        share_id: SHARE.to_vec(),
                    },
                    NOW,
                )
                .await
            ),
            None,
            "a fetch answers with a fetch, not a publication"
        );
    }

    /// The devices a grant named, or `None` when it did not grant anything.
    fn granted_devices(reply: &Envelope) -> Option<&Vec<ShareRecipientDevice>> {
        match reply.payload.as_ref() {
            Some(Payload::ShareAccessGranted(granted)) => Some(&granted.recipient_devices),
            _ => None,
        }
    }

    /// A grant that authorizes but cannot then read the member's devices is
    /// an outage rather than a refusal: the owner retries instead of
    /// concluding the member has none.
    #[tokio::test]
    async fn a_grant_that_cannot_read_the_members_devices_reports_an_outage() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let _member = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;

        state.store().set_devices_unavailable(true);
        let reply = grant(
            &owner,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;

        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::Internal,
                "the identity store is unavailable".to_owned()
            )
        );
        assert!(
            granted_devices(&reply).is_none(),
            "a refused grant names no devices"
        );
    }

    /// A member who is simply not connected is a different answer from one
    /// who may not read the share: the sender should retry, not give up.
    #[tokio::test]
    async fn a_handoff_to_an_offline_member_reports_it_as_unavailable() {
        let state = AppState::default();
        let owner = signed_in(&state, ADA, 1).await;
        let member = signed_in(&state, GRACE, 2).await;
        publish(&owner, &state, &request(), &publication(1, b"c"), NOW).await;
        grant(
            &owner,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;

        // Authorized, enrolled, and holding no live connection.
        let reply = handoff(
            &owner,
            &state,
            &request(),
            &ShareHandoff {
                share_id: SHARE.to_vec(),
                recipient_device_id: member
                    .identity()
                    .expect("signed in")
                    .device
                    .device_id
                    .to_vec(),
                ciphertext: b"magnet".to_vec(),
                info_hash: vec![7; 20],
            },
            NOW,
        )
        .await;

        assert_eq!(
            refusal(&reply).expect("a refusal"),
            (
                ProtocolErrorCode::Unavailable,
                "the recipient device is offline".to_owned()
            )
        );
    }

    /// Every domain refusal reaches the wire as a code the caller can act on,
    /// and a storage failure keeps its detail in the logs.
    #[tokio::test]
    async fn every_share_failure_maps_onto_a_typed_refusal() {
        let state = AppState::default();
        let session = signed_in(&state, ADA, 1).await;
        let oversized = publication(1, &vec![0; MAX_SHARE_CAPSULE_BYTES + 1]);

        for (command, expected) in [
            (oversized, ProtocolErrorCode::InvalidMessage),
            // Revision two with nothing published is a stale publisher.
            (publication(2, b"c"), ProtocolErrorCode::InvalidMessage),
        ] {
            let reply = publish(&session, &state, &request(), &command, NOW).await;
            assert_eq!(refusal(&reply).expect("a refusal").0, expected);
        }

        // Granting on a share nobody published cannot find it.
        let reply = grant(
            &session,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: GRACE.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::NotFound
        );

        publish(&session, &state, &request(), &publication(1, b"c"), NOW).await;

        // A member who was never registered cannot be granted access.
        let reply = grant(
            &session,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: [9; USER_ID_BYTES].to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::Unauthorized
        );

        // Someone else's share may be seeded but never moved or shared on.
        let stranger = signed_in(&state, GRACE, 2).await;
        let reply = grant(
            &stranger,
            &state,
            &request(),
            &GrantShareAccess {
                share_id: SHARE.to_vec(),
                member_user_id: ADA.to_vec(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            refusal(&reply).expect("a refusal").0,
            ProtocolErrorCode::Unauthorized
        );

        // Storage detail never reaches the wire.
        state.store().set_unavailable(true);
        for reply in [
            publish(&session, &state, &request(), &publication(1, b"c"), NOW).await,
            list(&session, &state, &request(), NOW).await,
            fetch(
                &session,
                &state,
                &request(),
                &FetchShareRequest {
                    share_id: SHARE.to_vec(),
                },
                NOW,
            )
            .await,
            grant(
                &session,
                &state,
                &request(),
                &GrantShareAccess {
                    share_id: SHARE.to_vec(),
                    member_user_id: GRACE.to_vec(),
                },
                NOW,
            )
            .await,
        ] {
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::Internal);
            assert_eq!(message, "the share store is unavailable");
        }
    }
}
