//! Retry, supervision, and recovery across connection loss.

use std::time::{Duration, Instant};

use portalis_nexus_client::{ClientConfig, NexusClient, TransportError};
use portalis_nexus_protocol::v1::Pong;
use portalis_nexus_protocol::v1::envelope::Payload;
use tokio::time::{sleep, timeout};

mod common;

use common::{
    PATIENCE, brisk_config, brisk_policy, closable_router, endpoint, reserve_address, serve,
    start_server, wait_until,
};

/// Retries until the supervisor has re-established a connection.
async fn ping_when_reconnected(client: &NexusClient, nonce: u64) -> Payload {
    let response = timeout(PATIENCE, async {
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
async fn connect_fails_fast_on_an_unreachable_endpoint() {
    let address = reserve_address().await;
    let started = Instant::now();

    let error = NexusClient::connect(&endpoint(address))
        .await
        .expect_err("nothing is listening");

    assert!(
        !matches!(error, TransportError::ReconnectExhausted { .. }),
        "connect makes a single attempt, got {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "connect should not spend a retry budget"
    );
}

#[tokio::test]
async fn exhausts_its_retry_budget_when_the_endpoint_never_appears() {
    let address = reserve_address().await;

    let error = NexusClient::connect_with_config(&endpoint(address), &brisk_config(3))
        .await
        .expect_err("nothing is listening");

    assert!(
        matches!(
            error,
            TransportError::ReconnectExhausted { attempts: 3, .. }
        ),
        "expected three attempts, got {error:?}"
    );
}

#[tokio::test]
async fn connects_once_a_late_server_appears() {
    let address = reserve_address().await;
    let target = endpoint(address);
    let connecting =
        tokio::spawn(
            async move { NexusClient::connect_with_config(&target, &brisk_config(200)).await },
        );

    sleep(Duration::from_millis(60)).await;
    let (_state, server) = start_server(address).await;

    let client = timeout(PATIENCE, connecting)
        .await
        .expect("connect resolves")
        .expect("connect task")
        .expect("connect succeeds once the server is up");
    assert!(client.is_connected());
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn supervised_clients_recover_from_a_forced_server_restart() {
    let address = reserve_address().await;
    let (_state, initial_server) = start_server(address).await;
    let config = brisk_config(u32::MAX);
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
async fn reports_connection_state_across_a_drop_and_recovery() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let client = NexusClient::connect_with_config(&endpoint(address), &brisk_config(u32::MAX))
        .await
        .expect("connect Nexus client");
    assert!(client.is_connected());

    // Draining closes the live socket and aborting stops the listener, so the
    // client cannot reconnect and stays observably disconnected.
    timeout(PATIENCE, state.shutdown().drain())
        .await
        .expect("drain closes the live socket");
    server.abort();
    let _ = server.await;
    wait_until("the dropped connection is reported", || {
        !client.is_connected()
    })
    .await;

    let (_state, restarted) = start_server(address).await;
    wait_until("the connection is restored", || client.is_connected()).await;

    assert_eq!(
        ping_when_reconnected(&client, 5).await,
        Payload::Pong(Pong { nonce: 5 })
    );
    client.shutdown().await;
    restarted.abort();
}

#[tokio::test]
async fn in_flight_requests_fail_as_soon_as_the_connection_drops() {
    let address = reserve_address().await;
    let (router, close) = closable_router();
    let server = serve(address, router).await;
    let config = ClientConfig {
        // Far longer than the test: the request must fail on disconnect, not
        // by timing out.
        request_timeout: Duration::from_secs(120),
        reconnect: brisk_policy(u32::MAX),
    };
    let client = NexusClient::connect_with_config(&endpoint(address), &config)
        .await
        .expect("connect to the closable server");

    let started = Instant::now();
    let (result, ()) = tokio::join!(client.ping(1), async {
        sleep(Duration::from_millis(50)).await;
        close.send_replace(true);
    });

    let error = result.expect_err("the connection ended first");
    assert!(
        matches!(error, TransportError::ConnectionClosed),
        "expected a closed connection, got {error:?}"
    );
    assert!(
        started.elapsed() < PATIENCE,
        "the request waited for its timeout instead of failing on disconnect"
    );
    assert_eq!(client.in_flight(), 0, "the waiter must not leak");
    server.abort();
}

#[tokio::test]
async fn shutdown_stops_the_supervisor() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect_with_config(&endpoint(address), &brisk_config(u32::MAX))
        .await
        .expect("connect Nexus client");

    timeout(PATIENCE, client.shutdown())
        .await
        .expect("shutdown closes the socket and joins the supervisor");

    server.abort();
}
