use portalis_nexus_client::NexusClient;
use portalis_nexus_protocol::v1::Pong;
use portalis_nexus_protocol::v1::envelope::Payload;

#[tokio::test]
async fn connects_and_exchanges_a_correlated_ping() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let state = portalis_nexus_server::AppState::default();
    state.mark_ready();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, portalis_nexus_server::app(&state)).await;
    });

    let mut client = NexusClient::connect(&format!("ws://{address}/v1/socket"))
        .await
        .expect("connect Nexus client");
    assert_eq!(client.hello().challenge.len(), 32);
    let response = client.ping(42, 1000).await.expect("receive pong");

    assert_eq!(response.correlation_id.len(), 16);
    assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 42 })));
    server.abort();
}
