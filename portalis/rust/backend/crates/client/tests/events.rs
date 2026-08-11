//! Server-initiated envelopes reaching the caller's event stream.

use portalis_nexus_client::NexusClient;
use portalis_nexus_protocol::v1::Ping;
use portalis_nexus_protocol::v1::envelope::Payload;
use tokio::time::timeout;

mod common;

use common::{PATIENCE, endpoint, event_router, reserve_address, serve, start_server};

#[tokio::test]
async fn delivers_unsolicited_envelopes_to_the_event_stream() {
    let address = reserve_address().await;
    let server = serve(address, event_router()).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect to the event server");
    let mut events = client.events().expect("the event stream is available");

    for nonce in 1..=3 {
        let event = timeout(PATIENCE, events.recv())
            .await
            .expect("an event arrives")
            .expect("the stream stays open");

        assert!(
            event.correlation_id.is_empty(),
            "an event answers no request"
        );
        assert_eq!(event.payload, Some(Payload::Ping(Ping { nonce })));
    }

    assert_eq!(client.in_flight(), 0);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn the_event_stream_is_taken_once() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    assert!(client.events().is_some());
    assert!(
        client.events().is_none(),
        "a second caller cannot steal the stream"
    );

    client.shutdown().await;
    server.abort();
}
