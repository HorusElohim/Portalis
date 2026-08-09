//! Handle resolution and friendships over a real socket.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::{ClientError, DeviceSigner, NexusClient, TransportError};
use portalis_nexus_protocol::v1::{FriendAction, FriendshipState, ProtocolErrorCode};
use portalis_nexus_protocol::{DEVICE_KEY_BYTES, SIGNATURE_BYTES};

mod common;

use common::{endpoint, reserve_address, start_server};

struct TestDevice(SigningKey);

impl DeviceSigner for TestDevice {
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.0.verifying_key().to_bytes()
    }

    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.0.sign(payload).to_bytes()
    }
}

fn device(seed: u8) -> TestDevice {
    TestDevice(SigningKey::from_bytes(&[seed; 32]))
}

fn refusal(error: &TransportError) -> ProtocolErrorCode {
    let TransportError::Client(ClientError::Refused { code, .. }) = error else {
        panic!("expected a typed refusal, got {error:?}");
    };
    *code
}

/// Connects and registers `username`, returning the client and its handle.
async fn registered(
    address: std::net::SocketAddr,
    username: &str,
    seed: u8,
) -> (NexusClient, String, Vec<u8>) {
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");
    let identity = client
        .register(username, &device(seed))
        .await
        .expect("registration succeeds");
    let handle = format!("{}#{}", identity.username, identity.discriminator);
    (client, handle, identity.user_id)
}

#[tokio::test]
async fn two_clients_become_friends() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, _ada_handle, ada_id) = registered(address, "ada", 7).await;
    let (grace, grace_handle, grace_id) = registered(address, "grace", 8).await;

    // Ada finds Grace by the handle a person would type.
    let found = ada
        .resolve_handle(&grace_handle)
        .await
        .expect("Grace resolves");
    assert_eq!(found.user_id, grace_id);
    assert_eq!(found.username, "grace");

    let requested = ada
        .friend_command(FriendAction::Request, &found.user_id)
        .await
        .expect("request sent");
    assert_eq!(requested.state, FriendshipState::Pending as i32);
    assert!(requested.requested_by_me);

    // Grace sees the request as someone else's, and accepts it.
    let waiting = grace.list_friends().await.expect("Grace lists");
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].user_id, ada_id);
    assert!(!waiting[0].requested_by_me);

    let accepted = grace
        .friend_command(FriendAction::Accept, &ada_id)
        .await
        .expect("request accepted");
    assert_eq!(accepted.state, FriendshipState::Accepted as i32);

    // Both sides now agree, from one edge.
    for (client, peer) in [(&ada, grace_id.clone()), (&grace, ada_id.clone())] {
        let friends = client.list_friends().await.expect("listed");
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].user_id, peer);
        assert_eq!(friends[0].state, FriendshipState::Accepted as i32);
    }

    ada.shutdown().await;
    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn only_the_recipient_may_accept() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, _, _) = registered(address, "ada", 7).await;
    let (grace, _, grace_id) = registered(address, "grace", 8).await;
    ada.friend_command(FriendAction::Request, &grace_id)
        .await
        .expect("request sent");

    let error = ada
        .friend_command(FriendAction::Accept, &grace_id)
        .await
        .expect_err("you cannot accept your own request");

    assert_eq!(refusal(&error), ProtocolErrorCode::InvalidMessage);
    ada.shutdown().await;
    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn removing_ends_the_friendship_for_both() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, _, ada_id) = registered(address, "ada", 7).await;
    let (grace, _, grace_id) = registered(address, "grace", 8).await;
    ada.friend_command(FriendAction::Request, &grace_id)
        .await
        .expect("request sent");
    grace
        .friend_command(FriendAction::Accept, &ada_id)
        .await
        .expect("accepted");

    let removed = grace
        .friend_command(FriendAction::Remove, &ada_id)
        .await
        .expect("removed");

    assert_eq!(removed.state, FriendshipState::Removed as i32);
    let ada_sees = ada.list_friends().await.expect("listed");
    assert_eq!(ada_sees[0].state, FriendshipState::Removed as i32);
    ada.shutdown().await;
    grace.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn friends_require_authentication() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let stranger = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");

    for error in [
        stranger
            .list_friends()
            .await
            .expect_err("not authenticated"),
        stranger
            .resolve_handle("ada#7Q2XZ")
            .await
            .expect_err("not authenticated"),
        stranger
            .friend_command(FriendAction::Request, &[1; 16])
            .await
            .expect_err("not authenticated"),
    ] {
        assert_eq!(refusal(&error), ProtocolErrorCode::Unauthenticated);
    }

    stranger.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn unknown_handles_and_peers_are_refused() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let (ada, _, _) = registered(address, "ada", 7).await;

    for error in [
        ada.resolve_handle("nobody#7Q2XZ")
            .await
            .expect_err("no such user"),
        ada.resolve_handle("not-a-handle")
            .await
            .expect_err("malformed"),
        ada.friend_command(FriendAction::Request, &[9; 16])
            .await
            .expect_err("no such peer"),
        ada.friend_command(FriendAction::Request, &[9; 4])
            .await
            .expect_err("not a user id"),
    ] {
        assert_eq!(refusal(&error), ProtocolErrorCode::InvalidMessage);
    }

    ada.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn an_answer_of_the_wrong_shape_is_rejected() {
    use common::{misanswering_router, serve};

    let address = reserve_address().await;
    let server = serve(address, misanswering_router()).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect");

    // The server answers every request with a pong, so nothing fits.
    for error in [
        client
            .resolve_handle("ada#7Q2XZ")
            .await
            .expect_err("a pong is not a resolved handle"),
        client
            .friend_command(FriendAction::Request, &[1; 16])
            .await
            .expect_err("a pong is not a friend event"),
        client
            .list_friends()
            .await
            .expect_err("a pong is not a friend list"),
    ] {
        assert!(
            matches!(
                error,
                TransportError::Client(ClientError::UnexpectedEnvelope { .. })
            ),
            "expected an unexpected-envelope error, got {error:?}"
        );
    }

    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn a_store_outage_is_reported_over_the_socket() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let (ada, _, _) = registered(address, "ada", 7).await;

    state.store().set_unavailable(true);

    for error in [
        ada.list_friends().await.expect_err("the store is down"),
        ada.resolve_handle("ada#7Q2XZ")
            .await
            .expect_err("the store is down"),
        ada.friend_command(FriendAction::Request, &[2; 16])
            .await
            .expect_err("the store is down"),
    ] {
        assert_eq!(refusal(&error), ProtocolErrorCode::Internal);
    }

    ada.shutdown().await;
    server.abort();
}
