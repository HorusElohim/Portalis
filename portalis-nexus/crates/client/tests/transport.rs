use std::net::SocketAddr;
use std::time::Duration;

use portalis_nexus_client::{NexusClient, ReconnectPolicy};
use portalis_nexus_protocol::v1::Pong;
use portalis_nexus_protocol::v1::envelope::Payload;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

async fn start_server(address: SocketAddr) -> JoinHandle<()> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind test server");
    let state = portalis_nexus_server::AppState::default();
    state.mark_ready();
    tokio::spawn(async move {
        let _ = axum::serve(listener, portalis_nexus_server::app(&state)).await;
    })
}

#[tokio::test]
async fn connects_and_exchanges_a_correlated_ping() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    drop(listener);
    let server = start_server(address).await;

    let mut client = NexusClient::connect(&format!("ws://{address}/v1/socket"))
        .await
        .expect("connect Nexus client");
    assert_eq!(client.hello().challenge.len(), 32);
    let response = client.ping(42, 1000).await.expect("receive pong");

    assert_eq!(response.correlation_id.len(), 16);
    assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 42 })));
    server.abort();
}

#[tokio::test]
async fn clients_reconnect_after_a_forced_server_restart() {
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve test server address");
    let address = reservation.local_addr().expect("test server address");
    drop(reservation);
    let initial_server = start_server(address).await;
    let endpoint = format!("ws://{address}/v1/socket");
    let initial_client = NexusClient::connect(&endpoint)
        .await
        .expect("connect before restart");
    drop(initial_client);
    initial_server.abort();
    let _ = initial_server.await;

    let policy = ReconnectPolicy::new(Duration::from_millis(20), Duration::from_millis(20), 20)
        .expect("valid reconnect policy");
    let first_endpoint = endpoint.clone();
    let first_policy = policy.clone();
    let first = tokio::spawn(async move {
        NexusClient::connect_with_retry(&first_endpoint, &first_policy).await
    });
    let second_endpoint = endpoint.clone();
    let second =
        tokio::spawn(
            async move { NexusClient::connect_with_retry(&second_endpoint, &policy).await },
        );

    sleep(Duration::from_millis(30)).await;
    let restarted_server = start_server(address).await;
    let mut first_client = timeout(Duration::from_secs(1), first)
        .await
        .expect("first reconnect completes")
        .expect("first reconnect task")
        .expect("first client reconnects");
    let mut second_client = timeout(Duration::from_secs(1), second)
        .await
        .expect("second reconnect completes")
        .expect("second reconnect task")
        .expect("second client reconnects");

    assert_eq!(
        first_client
            .ping(1, 1)
            .await
            .expect("first client pong")
            .payload,
        Some(Payload::Pong(Pong { nonce: 1 }))
    );
    assert_eq!(
        second_client
            .ping(2, 2)
            .await
            .expect("second client pong")
            .payload,
        Some(Payload::Pong(Pong { nonce: 2 }))
    );
    restarted_server.abort();
}
