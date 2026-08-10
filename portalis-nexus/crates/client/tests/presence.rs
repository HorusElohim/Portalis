//! Friend-only presence over real sockets.

use std::time::Duration;

use portalis_nexus_client::NexusClient;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{Envelope, FriendAction, PresenceEvent};
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

mod common;

use common::{PATIENCE, device, endpoint, reserve_address, start_server};

/// The presence in an envelope, or `None` when it carried none.
fn presence(envelope: &Envelope) -> Option<PresenceEvent> {
    match &envelope.payload {
        Some(Payload::PresenceEvent(event)) => Some(event.clone()),
        _ => None,
    }
}

/// Waits for the next presence event about `user`, ignoring anything else.
async fn next_presence(events: &mut Receiver<Envelope>, user: &[u8]) -> PresenceEvent {
    timeout(PATIENCE, async {
        loop {
            let envelope = events.recv().await.expect("the event stream stays open");
            if let Some(event) = presence(&envelope) {
                if event.user_id == user {
                    return event;
                }
            }
        }
    })
    .await
    .expect("a presence event arrives")
}

/// Connects, registers, and returns the client with its user id.
async fn registered(
    address: std::net::SocketAddr,
    username: &str,
    seed: u8,
) -> (NexusClient, Vec<u8>) {
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");
    let identity = client
        .register(username, &device(seed))
        .await
        .expect("registration succeeds");
    (client, identity.user_id)
}

/// Makes two registered users accepted friends.
async fn befriend(ada: &NexusClient, ada_id: &[u8], grace: &NexusClient, grace_id: &[u8]) {
    ada.friend_command(FriendAction::Request, grace_id)
        .await
        .expect("request sent");
    grace
        .friend_command(FriendAction::Accept, ada_id)
        .await
        .expect("accepted");
}

#[tokio::test]
async fn friends_see_each_other_go_offline_and_come_back() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, ada_id) = registered(address, "ada", 7).await;
    let (grace, grace_id) = registered(address, "grace", 8).await;
    befriend(&ada, &ada_id, &grace, &grace_id).await;
    let mut grace_events = grace.events().expect("Grace's event stream");

    // Ada's last device leaves.
    ada.shutdown().await;
    let offline = next_presence(&mut grace_events, &ada_id).await;
    assert!(!offline.online);
    assert!(
        offline.last_seen_unix_ms.is_some(),
        "someone offline was last seen at a time"
    );

    // Ada comes back on a new connection and authenticates.
    let returning = NexusClient::connect(&endpoint(address))
        .await
        .expect("reconnect");
    returning
        .authenticate(&device(7))
        .await
        .expect("authentication succeeds");

    let online = next_presence(&mut grace_events, &ada_id).await;
    assert!(online.online);
    assert_eq!(
        online.last_seen_unix_ms, None,
        "someone online has no last-seen time"
    );

    returning.shutdown().await;
    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn arriving_learns_where_friends_already_stand() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, ada_id) = registered(address, "ada", 7).await;
    let (grace, grace_id) = registered(address, "grace", 8).await;
    befriend(&ada, &ada_id, &grace, &grace_id).await;

    // Grace opens a second device while Ada is already connected.
    let laptop = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");
    let mut laptop_events = laptop.events().expect("the laptop's event stream");
    laptop
        .authenticate(&device(8))
        .await
        .expect("authentication succeeds");

    let ada_state = next_presence(&mut laptop_events, &ada_id).await;

    assert!(
        ada_state.online,
        "a new connection is told who is already online"
    );
    laptop.shutdown().await;
    ada.shutdown().await;
    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn presence_is_not_shared_with_strangers() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, ada_id) = registered(address, "ada", 7).await;
    let (stranger, _) = registered(address, "mallory", 9).await;
    let mut watching = stranger.events().expect("the stranger's event stream");

    ada.shutdown().await;

    // Nothing about Ada reaches someone she never befriended.
    let leaked = timeout(Duration::from_millis(300), watching.recv()).await;
    assert!(
        leaked.is_err(),
        "presence reached a stranger: {:?}",
        leaked.map(|event| event.and_then(|envelope| presence(&envelope)))
    );
    let _ = ada_id;

    stranger.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn a_pending_friendship_shares_nothing() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, _ada_id) = registered(address, "ada", 7).await;
    let (grace, grace_id) = registered(address, "grace", 8).await;
    // Requested, never accepted.
    ada.friend_command(FriendAction::Request, &grace_id)
        .await
        .expect("request sent");
    let mut grace_events = grace.events().expect("Grace's event stream");

    ada.shutdown().await;

    let leaked = timeout(Duration::from_millis(300), grace_events.recv()).await;
    assert!(leaked.is_err(), "a pending friendship must share nothing");

    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn one_device_leaving_does_not_take_a_user_offline() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada_phone, ada_id) = registered(address, "ada", 7).await;
    let (grace, grace_id) = registered(address, "grace", 8).await;
    befriend(&ada_phone, &ada_id, &grace, &grace_id).await;

    // Ada's second device.
    let ada_laptop = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");
    ada_laptop
        .authenticate(&device(7))
        .await
        .expect("authentication succeeds");
    let mut grace_events = grace.events().expect("Grace's event stream");

    ada_phone.shutdown().await;

    // Ada is still online through the laptop, so nothing is announced.
    let announced = timeout(Duration::from_millis(300), grace_events.recv()).await;
    assert!(
        announced.is_err(),
        "one device leaving must not report the user offline"
    );

    ada_laptop.shutdown().await;
    grace.shutdown().await;
    server.abort();
}
