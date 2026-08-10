//! Telling friends who is online.
//!
//! Presence is private. Every event here is addressed to a specific accepted
//! friend, so it is never broadcast and never reaches someone a user has not
//! agreed to share it with. A pending or removed friendship sees nothing.

use portalis_nexus_protocol::v1::FriendshipState;
use portalis_nexus_server_core::{ConnectionId, UserId};
use tracing::debug;

use crate::messages::{binary_frame, presence_event};
use crate::state::AppState;

/// Tells `user`'s accepted friends that they came online or went away.
///
/// Their own other devices are told too, so a phone reflects what a laptop
/// just did.
pub(crate) async fn announce(state: &AppState, user: UserId, online: bool, now_unix_ns: u64) {
    let last_seen = if online {
        None
    } else {
        state.presence().last_seen(user)
    };
    let event = binary_frame(&presence_event(user, online, last_seen, now_unix_ns));

    for audience in accepted_friends(state, user).await {
        for connection in state.presence().connections_of(audience) {
            state.connections().send(connection, event.clone());
        }
    }
}

/// Tells a newly arrived connection where its friends stand.
///
/// Without this a client would see nothing until someone's state changed,
/// which could be hours.
pub(crate) async fn greet(
    state: &AppState,
    user: UserId,
    connection: ConnectionId,
    now_unix_ns: u64,
) {
    for friend in accepted_friends(state, user).await {
        let online = state.presence().is_online(friend);
        let event = presence_event(
            friend,
            online,
            if online {
                None
            } else {
                state.presence().last_seen(friend)
            },
            now_unix_ns,
        );
        state.connections().send(connection, binary_frame(&event));
    }
}

/// The users allowed to see `user`'s presence: accepted friends only.
async fn accepted_friends(state: &AppState, user: UserId) -> Vec<UserId> {
    match state.friends().list(user).await {
        Ok(friends) => friends
            .into_iter()
            .filter(|friend| friend.state == FriendshipState::Accepted)
            .map(|friend| friend.peer.user_id)
            .collect(),
        Err(error) => {
            // Presence is best-effort: a store outage must not fail the
            // command that triggered it, and clients refresh on reconnect.
            debug!(%error, "could not read friends for a presence event");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::v1::envelope::Payload;
    use portalis_nexus_protocol::{decode_frame, v1::PresenceEvent};
    use portalis_nexus_server_core::{
        DeviceRecord, FriendRepository, FriendshipEdge, FriendshipRecord, IdentityRepository,
        UserRecord,
    };
    use tokio::sync::mpsc;

    use super::*;

    const ADA: UserId = [1; 16];
    const GRACE: UserId = [2; 16];
    const GRACE_PHONE: ConnectionId = [20; 16];
    const NOW: u64 = 1_700_000_000_000_000_000;

    fn user(id: UserId, username: &str) -> UserRecord {
        UserRecord {
            user_id: id,
            username: username.to_owned(),
            normalized_username: username.to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        }
    }

    /// A server where Ada and Grace are accepted friends, and Grace has a
    /// connection whose outbound queue the test can read.
    async fn befriended() -> (AppState, mpsc::Receiver<axum::extract::ws::Message>) {
        let state = AppState::default();
        for (id, name) in [(ADA, "ada"), (GRACE, "grace")] {
            state
                .store()
                .insert_registration(
                    user(id, name),
                    DeviceRecord {
                        device_id: [id[0]; 32],
                        user_id: id,
                        public_key: [id[0]; 32],
                        encryption_public_key: [id[0]; 32],
                        created_at_unix_ns: NOW,
                        last_authenticated_at_unix_ns: Some(NOW),
                        revoked_at_unix_ns: None,
                    },
                )
                .await
                .expect("seeded");
        }
        let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct");
        let mut friendship = FriendshipRecord::requested(edge, ADA, NOW);
        friendship.state = FriendshipState::Accepted;
        state
            .store()
            .save_friendship(friendship, 0)
            .await
            .expect("friendship seeded");

        let (outbound, inbox) = mpsc::channel(8);
        state.connections().register(GRACE_PHONE, outbound);
        let _ = state.presence().arrive(GRACE, GRACE_PHONE);
        (state, inbox)
    }

    /// The next payload queued for a connection, or `None` when none is.
    fn received(inbox: &mut mpsc::Receiver<axum::extract::ws::Message>) -> Option<Payload> {
        let frame = inbox.try_recv().ok()?.into_data();
        decode_frame(&frame)
            .expect("a server frame is valid")
            .payload
    }

    /// The payload a presence announcement should carry.
    fn presence_of(user: UserId, online: bool, last_seen_unix_ns: Option<u64>) -> Payload {
        Payload::PresenceEvent(PresenceEvent {
            user_id: user.to_vec(),
            online,
            last_seen_unix_ns,
        })
    }

    #[tokio::test]
    async fn arriving_is_told_a_friend_is_away() {
        let (state, mut inbox) = befriended().await;

        // Ada has never connected, so Grace should hear she is offline.
        greet(&state, GRACE, GRACE_PHONE, NOW).await;

        assert_eq!(
            received(&mut inbox),
            Some(presence_of(ADA, false, None)),
            "Ada is away and has never been seen"
        );
    }

    #[tokio::test]
    async fn arriving_is_told_a_friend_is_here_and_since_when() {
        let (state, mut inbox) = befriended().await;
        let ada_phone: ConnectionId = [21; 16];
        let _ = state.presence().arrive(ADA, ada_phone);

        greet(&state, GRACE, GRACE_PHONE, NOW).await;
        assert_eq!(received(&mut inbox), Some(presence_of(ADA, true, None)));

        // Ada leaves, and the next arrival hears when she was last seen.
        let _ = state.presence().depart(ADA, ada_phone, NOW + 5);
        greet(&state, GRACE, GRACE_PHONE, NOW + 6).await;
        assert_eq!(
            received(&mut inbox),
            Some(presence_of(ADA, false, Some(NOW + 5)))
        );
    }

    #[tokio::test]
    async fn a_change_reaches_an_accepted_friend() {
        let (state, mut inbox) = befriended().await;

        announce(&state, ADA, true, NOW).await;

        assert_eq!(received(&mut inbox), Some(presence_of(ADA, true, None)));
    }

    #[tokio::test]
    async fn a_store_outage_shares_nothing_rather_than_failing() {
        let (state, mut inbox) = befriended().await;
        state.store().set_unavailable(true);

        // Neither call may panic or block; presence is best-effort.
        announce(&state, ADA, false, NOW).await;
        greet(&state, GRACE, GRACE_PHONE, NOW).await;

        assert!(
            received(&mut inbox).is_none(),
            "nothing is sent when friends cannot be read"
        );
    }
}
