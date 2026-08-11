//! Handle resolution, friend commands, and friend listing.

use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, Friend, FriendCommand, FriendEvent, ListFriendsResponse, ProtocolErrorCode,
    ResolveHandleRequest, ResolveHandleResponse,
};
use portalis_nexus_server_core::{FriendError, FriendSummary, UserId};

use crate::identity::{DefaultStore, NexusFriends};
use crate::messages::{protocol_error, reply_with};
use crate::session::Session;

/// Answers a lookup of `<username>#<discriminator>`.
pub(crate) async fn resolve(
    session: &Session,
    friends: &NexusFriends<DefaultStore>,
    request: &Envelope,
    lookup: &ResolveHandleRequest,
    now_unix_ns: u64,
) -> Envelope {
    if actor(session).is_none() {
        return unauthenticated(request, now_unix_ns);
    }
    match friends.resolve_handle(&lookup.handle).await {
        Ok(found) => reply_with(
            request,
            Payload::ResolveHandleResponse(ResolveHandleResponse {
                user_id: found.user_id.to_vec(),
                username: found.username,
                discriminator: found.discriminator,
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

/// Applies one friend action and reports where the friendship stands.
pub(crate) async fn command(
    session: &Session,
    friends: &NexusFriends<DefaultStore>,
    request: &Envelope,
    command: &FriendCommand,
    now_unix_ns: u64,
) -> Envelope {
    let Some(actor) = actor(session) else {
        return unauthenticated(request, now_unix_ns);
    };
    let Ok(peer) = UserId::try_from(command.peer_user_id.as_slice()) else {
        return protocol_error(
            ProtocolErrorCode::InvalidMessage,
            request.message_id.clone(),
            "peer_user_id must name a user".to_owned(),
            now_unix_ns,
        );
    };

    match friends.command_summary(actor, peer, command.action()).await {
        Ok(summary) => reply_with(
            request,
            Payload::FriendEvent(FriendEvent {
                friend: Some(friend_of(&summary)),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

/// Lists every friendship the asking user is part of.
pub(crate) async fn list(
    session: &Session,
    friends: &NexusFriends<DefaultStore>,
    request: &Envelope,
    now_unix_ns: u64,
) -> Envelope {
    let Some(actor) = actor(session) else {
        return unauthenticated(request, now_unix_ns);
    };
    match friends.list(actor).await {
        Ok(summaries) => reply_with(
            request,
            Payload::ListFriendsResponse(ListFriendsResponse {
                friends: summaries.iter().map(friend_of).collect(),
            }),
            now_unix_ns,
        ),
        Err(error) => rejection(request, &error, now_unix_ns),
    }
}

/// The identity a command acts as.
///
/// Every friend command names a user, so a connection that has not proved who
/// it is has nothing to act as.
fn actor(session: &Session) -> Option<UserId> {
    session.identity().map(|identity| identity.user.user_id)
}

/// Refuses a command from a connection that has not proved who it is.
fn unauthenticated(request: &Envelope, now_unix_ns: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        "authenticate before using friends".to_owned(),
        now_unix_ns,
    )
}

/// Renders one side of a friendship for the wire.
fn friend_of(summary: &FriendSummary) -> Friend {
    Friend {
        user_id: summary.peer.user_id.to_vec(),
        username: summary.peer.username.clone(),
        discriminator: summary.peer.discriminator.clone(),
        state: summary.state as i32,
        requested_by_me: summary.requested_by_me,
    }
}

/// Maps a friend failure onto the wire, keeping storage detail off it.
fn rejection(request: &Envelope, error: &FriendError, now_unix_ns: u64) -> Envelope {
    let code = match error {
        FriendError::Repository(_) => ProtocolErrorCode::Internal,
        // Losing every retry means the edge is being hammered, not that the
        // request was wrong; the caller should try again shortly.
        FriendError::Contended => ProtocolErrorCode::RateLimited,
        FriendError::Handle(_) | FriendError::Friendship(_) | FriendError::UnknownUser => {
            ProtocolErrorCode::InvalidMessage
        }
    };
    let message = match error {
        FriendError::Repository(_) => "the friend store is unavailable".to_owned(),
        other => other.to_string(),
    };
    protocol_error(code, request.message_id.clone(), message, now_unix_ns)
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::new_message_id;
    use portalis_nexus_protocol::v1::{Ping, ProtocolError};
    use portalis_nexus_server_core::{
        FriendAction, FriendshipError, FriendshipState, HandleError, RepositoryError, UserRecord,
    };

    use super::*;
    use crate::state::AppState;

    const NOW: u64 = 1_700_000_000_000_000_000;

    fn request() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        }
    }

    /// The friend in a reply, or `None` when it carried none.
    fn friend_in(reply: &Envelope) -> Option<Friend> {
        match &reply.payload {
            Some(Payload::FriendEvent(FriendEvent { friend })) => friend.clone(),
            _ => None,
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

    #[test]
    fn every_friend_failure_maps_onto_a_typed_refusal() {
        let request = request();

        for (error, expected) in [
            (
                FriendError::Handle(HandleError::Malformed),
                ProtocolErrorCode::InvalidMessage,
            ),
            (
                FriendError::Friendship(FriendshipError::NotTheRecipient),
                ProtocolErrorCode::InvalidMessage,
            ),
            (FriendError::UnknownUser, ProtocolErrorCode::InvalidMessage),
            // Losing every retry is a timing problem, not a bad request.
            (FriendError::Contended, ProtocolErrorCode::RateLimited),
        ] {
            let reply = rejection(&request, &error, NOW);

            assert_eq!(
                refusal(&reply).map(|(code, _)| code),
                Some(expected),
                "for {error}"
            );
            assert_eq!(reply.correlation_id, request.message_id);
        }
    }

    #[test]
    fn storage_detail_never_reaches_the_wire() {
        let outage = FriendError::Repository(RepositoryError::Unavailable(
            "connection refused to db-1.internal".to_owned(),
        ));

        let reply = rejection(&request(), &outage, NOW);

        let (code, message) = refusal(&reply).expect("a refusal");
        assert_eq!(code, ProtocolErrorCode::Internal);
        assert_eq!(message, "the friend store is unavailable");
        assert!(!message.contains("db-1.internal"));
        assert!(refusal(&request()).is_none());
    }

    #[test]
    fn an_unauthenticated_command_is_refused() {
        let reply = unauthenticated(&request(), NOW);

        assert_eq!(
            refusal(&reply).map(|(code, _)| code),
            Some(ProtocolErrorCode::Unauthenticated)
        );
    }

    #[test]
    fn a_friendship_renders_the_peer_and_who_asked() {
        let summary = FriendSummary {
            peer: UserRecord {
                user_id: [2; 16],
                username: "Grace".to_owned(),
                normalized_username: "grace".to_owned(),
                discriminator: "ABCDE".to_owned(),
                created_at_unix_ns: NOW,
            },
            state: FriendshipState::Accepted,
            requested_by_me: true,
        };

        let friend = friend_of(&summary);

        assert_eq!(friend.user_id, vec![2; 16]);
        assert_eq!(friend.username, "Grace");
        assert_eq!(friend.discriminator, "ABCDE");
        assert_eq!(friend.state, FriendshipState::Accepted as i32);
        assert!(friend.requested_by_me);
    }

    /// A connection already bound to a registered user.
    async fn signed_in(state: &AppState, username: &str, seed: u8) -> Session {
        use portalis_nexus_server_core::{DeviceRecord, IdentityRepository, UserRecord};

        let user = UserRecord {
            user_id: [seed; 16],
            username: username.to_owned(),
            normalized_username: username.to_lowercase(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        };
        let device = DeviceRecord {
            device_id: [seed; 32],
            user_id: [seed; 16],
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

        let policy = portalis_nexus_server_core::ProtocolPolicy::new(1, 1).expect("range");
        let mut session = Session::new(&crate::messages::hello_payload(&policy, NOW));
        session.bind(portalis_nexus_server_core::Identity { user, device });
        session
    }

    #[tokio::test]
    async fn a_store_outage_is_reported_by_every_command() {
        let state = AppState::default();
        let session = signed_in(&state, "ada", 1).await;
        state.store().set_unavailable(true);

        let replies = [
            resolve(
                &session,
                state.friends(),
                &request(),
                &ResolveHandleRequest {
                    handle: "grace#ABCDE".to_owned(),
                },
                NOW,
            )
            .await,
            command(
                &session,
                state.friends(),
                &request(),
                &FriendCommand {
                    action: 1,
                    peer_user_id: vec![2; 16],
                },
                NOW,
            )
            .await,
            list(&session, state.friends(), &request(), NOW).await,
        ];

        for reply in replies {
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::Internal);
            assert_eq!(message, "the friend store is unavailable");
        }
    }

    #[tokio::test]
    async fn a_signed_in_connection_can_command_list_and_resolve() {
        let state = AppState::default();
        let session = signed_in(&state, "ada", 1).await;
        signed_in(&state, "grace", 2).await;

        let requested = command(
            &session,
            state.friends(),
            &request(),
            &FriendCommand {
                action: FriendAction::Request as i32,
                peer_user_id: vec![2; 16],
            },
            NOW,
        )
        .await;
        let friend = friend_in(&requested).expect("a friend event");
        assert_eq!(friend.username, "grace");
        assert!(friend.requested_by_me);

        let listed = list(&session, state.friends(), &request(), NOW).await;
        assert!(friend_in(&listed).is_none(), "a listing is not one event");
        assert_eq!(
            listed.payload,
            Some(Payload::ListFriendsResponse(ListFriendsResponse {
                friends: vec![friend]
            }))
        );

        let resolved = resolve(
            &session,
            state.friends(),
            &request(),
            &ResolveHandleRequest {
                handle: "ada#7Q2XZ".to_owned(),
            },
            NOW,
        )
        .await;
        assert_eq!(
            resolved.payload,
            Some(Payload::ResolveHandleResponse(ResolveHandleResponse {
                user_id: vec![1; 16],
                username: "ada".to_owned(),
                discriminator: "7Q2XZ".to_owned(),
            }))
        );
    }
}
