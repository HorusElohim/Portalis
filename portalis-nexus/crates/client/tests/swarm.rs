//! M5 deterministic discovery over source-address-bound short leases.

use portalis_nexus_client::NexusClient;
use portalis_nexus_protocol::v1::AddressFamily;

mod common;

use common::{device, endpoint, reserve_address, start_server, wait_until};

const INFO_HASH: [u8; 20] = [7; 20];

#[tokio::test]
async fn peers_discover_current_endpoints_and_disconnect_removes_them() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let first_device = device(41);
    let second_device = device(42);
    let first = NexusClient::connect(&endpoint(address))
        .await
        .expect("first connects");
    let second = NexusClient::connect(&endpoint(address))
        .await
        .expect("second connects");
    first
        .register("Alice", &first_device)
        .await
        .expect("first registers");
    second
        .register("Bob", &second_device)
        .await
        .expect("second registers");

    first
        .announce_peer(&INFO_HASH, 6881, AddressFamily::Ipv4, 1, 90)
        .await
        .expect("first announces");
    second
        .announce_peer(&INFO_HASH, 6882, AddressFamily::Ipv4, 1, 90)
        .await
        .expect("second announces");

    let found = first
        .lookup_peers(&INFO_HASH, AddressFamily::Ipv4, 1)
        .await
        .expect("lookup");
    assert_eq!(found.peers.len(), 1);
    assert_eq!(found.peers[0].ip_address, [127, 0, 0, 1]);
    assert_eq!(found.peers[0].port, 6882);

    second
        .withdraw_peer(&INFO_HASH)
        .await
        .expect("explicit withdrawal");
    assert!(
        first
            .lookup_peers(&INFO_HASH, AddressFamily::Ipv4, 1)
            .await
            .expect("lookup after withdrawal")
            .peers
            .is_empty()
    );
    second
        .announce_peer(&INFO_HASH, 6882, AddressFamily::Ipv4, 1, 90)
        .await
        .expect("second re-announces");

    second.shutdown().await;
    wait_until("the disconnected peer lease is removed", || {
        state.swarm().len() == 1
    })
    .await;
    let after = first
        .lookup_peers(&INFO_HASH, AddressFamily::Ipv4, 1)
        .await
        .expect("lookup after disconnect");
    assert!(after.peers.is_empty());

    first.shutdown().await;
    server.abort();
}
