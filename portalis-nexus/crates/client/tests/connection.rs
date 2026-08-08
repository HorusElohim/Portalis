//! Handshake, request correlation, and connection teardown.

use std::sync::Arc;
use std::time::Duration;

use portalis_nexus_client::{ClientConfig, ClientError, NexusClient, TransportError};
use portalis_nexus_protocol::MAX_PENDING_REQUESTS;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{Envelope, Pong};
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, new_message_id};
use tokio::time::timeout;

mod common;

use common::{
    PATIENCE, bare_router, endpoint, future_router, reserve_address, serve, silent_router,
    start_server, wait_until,
};

#[tokio::test]
async fn establishes_a_validated_session() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;

    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let hello = client.hello().expect("a live connection has a hello");
    assert_eq!(hello.challenge.len(), 32);
    assert_eq!(hello.connection_id.len(), 16);
    let range = hello.supported_protocols.expect("protocol range");
    assert!((range.minimum..=range.maximum).contains(&CURRENT_PROTOCOL_VERSION));
    assert!(client.is_connected());
    assert_eq!(client.in_flight(), 0);

    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn exchanges_a_correlated_ping() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let response = client.ping(42).await.expect("receive pong");

    assert_eq!(response.correlation_id.len(), 16);
    assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 42 })));
    assert_eq!(client.in_flight(), 0);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn correlates_concurrent_requests_independently() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let responses = futures_util::future::join_all((0..32).map(|nonce| client.ping(nonce))).await;

    for (nonce, response) in responses.into_iter().enumerate() {
        let nonce = u64::try_from(nonce).expect("small nonce");
        assert_eq!(
            response.expect("pong").payload,
            Some(Payload::Pong(Pong { nonce })),
            "response {nonce} was correlated to the wrong request"
        );
    }
    assert_eq!(client.in_flight(), 0);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn relays_a_correlated_protocol_error_to_its_caller() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");
    // A Pong is not a command the server accepts before authentication.
    let unsupported = Envelope {
        message_id: new_message_id(),
        correlation_id: Vec::new(),
        sent_at_unix_ms: 1,
        payload: Some(Payload::Pong(Pong { nonce: 1 })),
    };

    let response = client
        .request(&unsupported)
        .await
        .expect("a protocol error is still a correlated response");

    assert_eq!(response.correlation_id, unsupported.message_id);
    assert!(
        matches!(response.payload, Some(Payload::ProtocolError(_))),
        "expected a protocol error, got {:?}",
        response.payload
    );
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn rejects_a_server_that_skips_the_protobuf_subprotocol() {
    let address = reserve_address().await;
    let server = serve(address, bare_router()).await;

    let error = NexusClient::connect(&endpoint(address))
        .await
        .expect_err("the subprotocol is mandatory");

    assert!(
        matches!(error, TransportError::WebSocket(_)),
        "expected the handshake to reject the subprotocol, got {error:?}"
    );
    server.abort();
}

#[tokio::test]
async fn rejects_a_server_speaking_an_unsupported_protocol_version() {
    let address = reserve_address().await;
    let server = serve(address, future_router()).await;

    let error = NexusClient::connect(&endpoint(address))
        .await
        .expect_err("the protocol range excludes this client");

    assert!(
        matches!(
            error,
            TransportError::Client(ClientError::UnsupportedProtocolVersion)
        ),
        "expected an unsupported version, got {error:?}"
    );
    server.abort();
}

#[tokio::test]
async fn rejects_a_malformed_endpoint() {
    let error = NexusClient::connect("not-a-websocket-url")
        .await
        .expect_err("the endpoint is not a WebSocket URL");

    assert!(
        matches!(error, TransportError::WebSocket(_)),
        "expected a WebSocket error, got {error:?}"
    );
}

#[tokio::test]
async fn refuses_more_than_the_pending_request_limit() {
    let address = reserve_address().await;
    let server = serve(address, silent_router()).await;
    let config = ClientConfig {
        request_timeout: Duration::from_secs(30),
        ..ClientConfig::default()
    };
    let client = Arc::new(
        NexusClient::connect_with_config(&endpoint(address), &config)
            .await
            .expect("connect to the silent server"),
    );

    let stalled: Vec<_> = (0..MAX_PENDING_REQUESTS)
        .map(|nonce| {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.ping(nonce as u64).await })
        })
        .collect();
    wait_until("every request is registered", || {
        client.in_flight() == MAX_PENDING_REQUESTS
    })
    .await;

    let error = client
        .ping(u64::MAX)
        .await
        .expect_err("the registry is full");

    assert!(
        matches!(
            error,
            TransportError::Client(ClientError::TooManyPendingRequests)
        ),
        "expected a full registry, got {error:?}"
    );
    for request in stalled {
        request.abort();
    }
    server.abort();
}

#[tokio::test]
async fn draining_the_server_closes_live_connections() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect before draining");

    timeout(PATIENCE, state.shutdown().drain())
        .await
        .expect("drain completes while a client is connected");

    assert!(state.shutdown().is_draining());
    assert!(
        client.ping(1).await.is_err(),
        "a drained connection cannot answer"
    );
    client.shutdown().await;
    server.abort();
}
