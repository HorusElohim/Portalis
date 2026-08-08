//! Real client/server transport tests over live sockets.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::response::Response;
use axum::routing::get;
use portalis_nexus_client::{ClientConfig, NexusClient, ReconnectPolicy, TransportError};
use portalis_nexus_protocol::WEBSOCKET_SUBPROTOCOL;
use portalis_nexus_protocol::v1::Pong;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_server::{AppState, binary_frame, server_hello};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

/// Reserves an ephemeral port so a restarted server can rebind the address.
async fn reserve_address() -> SocketAddr {
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve test server address");
    let address = reservation.local_addr().expect("test server address");
    drop(reservation);
    address
}

async fn serve(address: SocketAddr, router: Router) -> JoinHandle<()> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind test server");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    })
}

async fn start_server(address: SocketAddr) -> (AppState, JoinHandle<()>) {
    let state = AppState::default();
    state.mark_ready();
    let handle = serve(address, portalis_nexus_server::app(&state)).await;
    (state, handle)
}

fn endpoint(address: SocketAddr) -> String {
    format!("ws://{address}/v1/socket")
}

/// Greets a client correctly and then never answers another message.
async fn silent_upgrade(websocket: WebSocketUpgrade) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(|mut socket| async move {
            let policy = AppState::default();
            let hello = binary_frame(&server_hello(policy.protocol_policy(), 0));
            let _ = socket.send(hello).await;
            std::future::pending::<()>().await;
        })
}

/// Retries until the supervisor has re-established a connection.
async fn ping_when_reconnected(client: &NexusClient, nonce: u64) -> Payload {
    let response = timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(response) = client.ping(nonce).await {
                return response;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("client re-establishes its connection");
    response.payload.expect("pong payload")
}

#[tokio::test]
async fn connects_and_exchanges_a_correlated_ping() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;

    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    assert!(client.is_connected());
    assert_eq!(client.hello().expect("hello").challenge.len(), 32);
    let response = client.ping(42).await.expect("receive pong");
    assert_eq!(response.correlation_id.len(), 16);
    assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 42 })));

    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn concurrent_requests_are_correlated_independently() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let responses = futures_util::future::join_all((0..16).map(|nonce| client.ping(nonce))).await;

    for (nonce, response) in responses.into_iter().enumerate() {
        let nonce = u64::try_from(nonce).expect("small nonce");
        assert_eq!(
            response.expect("pong").payload,
            Some(Payload::Pong(Pong { nonce }))
        );
    }
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn supervised_clients_recover_from_a_forced_server_restart() {
    let address = reserve_address().await;
    let (_state, initial_server) = start_server(address).await;
    let config = ClientConfig {
        reconnect: ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(20),
            u32::MAX,
        )
        .expect("valid reconnect policy"),
        ..ClientConfig::default()
    };
    let first = NexusClient::connect_with_config(&endpoint(address), &config)
        .await
        .expect("first client connects");
    let second = NexusClient::connect_with_config(&endpoint(address), &config)
        .await
        .expect("second client connects");
    assert_eq!(
        first.ping(1).await.expect("pong").payload,
        Some(Payload::Pong(Pong { nonce: 1 }))
    );

    initial_server.abort();
    let _ = initial_server.await;
    let (_restarted_state, restarted_server) = start_server(address).await;

    // Neither caller reconnects; each supervisor restores its own connection.
    assert_eq!(
        ping_when_reconnected(&first, 7).await,
        Payload::Pong(Pong { nonce: 7 })
    );
    assert_eq!(
        ping_when_reconnected(&second, 9).await,
        Payload::Pong(Pong { nonce: 9 })
    );

    first.shutdown().await;
    second.shutdown().await;
    restarted_server.abort();
}

#[tokio::test]
async fn requests_time_out_when_the_server_never_answers() {
    let address = reserve_address().await;
    let server = serve(
        address,
        Router::new().route("/v1/socket", get(silent_upgrade)),
    )
    .await;
    let config = ClientConfig {
        request_timeout: Duration::from_millis(150),
        ..ClientConfig::default()
    };
    let client = NexusClient::connect_with_config(&endpoint(address), &config)
        .await
        .expect("connect to the silent server");

    let error = client.ping(1).await.expect_err("ping times out");

    assert!(
        matches!(error, TransportError::RequestTimeout(limit) if limit == config.request_timeout),
        "expected a request timeout, got {error:?}"
    );
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn draining_closes_live_connections() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect before draining");

    timeout(Duration::from_secs(5), state.shutdown().drain())
        .await
        .expect("drain completes while a client is connected");

    assert!(state.shutdown().is_draining());
    assert!(client.ping(1).await.is_err());
    client.shutdown().await;
    server.abort();
}
