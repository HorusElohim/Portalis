//! A guided tour of Portalis Nexus, server and clients in one process.
//!
//! Run it with `cargo run -p portalis-nexus-demo`. Nothing is mocked: this is
//! the real server, the real portable client, and real sockets between them.

use std::error::Error;
use std::net::SocketAddr;

use portalis_nexus_client::{ClientError, DeviceSigner, NexusClient, TransportError};
use portalis_nexus_demo::{DemoDevice, short};
use portalis_nexus_protocol::v1::AddressFamily;
use portalis_nexus_server::{AppState, GRACEFUL_DRAIN_TIMEOUT};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Stands in for the torrent descriptor a real capsule carries.
const MAGNET: &[u8] = b"magnet:?xt=urn:btih:0102030405060708090a0b0c0d0e0f1011121314";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (address, state, server) = start_server().await?;
    let endpoint = format!("ws://{address}/v1/socket");
    step(1, "A server is listening", &endpoint);

    // 1. Connect and inspect the greeting.
    let ada_device = DemoDevice::ephemeral(7);
    let ada = NexusClient::connect(&endpoint).await?;
    let hello = ada.hello().ok_or("the connection should be live")?;
    let range = hello
        .supported_protocols
        .ok_or("a validated hello carries a protocol range")?;
    step(
        2,
        "A client connected and was greeted",
        &format!(
            "connection {}, a {}-byte challenge, protocol {}..={}",
            short(&hello.connection_id),
            hello.challenge.len(),
            range.minimum,
            range.maximum
        ),
    );

    // 2. Register, which claims a handle and enrols this device.
    let identity = ada.register("ada", &ada_device).await?;
    step(
        3,
        "That client registered",
        &format!(
            "{}#{} is user {}, device {}",
            identity.username,
            identity.discriminator,
            short(&identity.user_id),
            short(&identity.device_id)
        ),
    );

    // 3. A different device asking for the same name gets its own handle.
    let grace_device = DemoDevice::ephemeral(9);
    let grace = NexusClient::connect(&endpoint).await?;
    let shared_name = grace.register("ada", &grace_device).await?;
    step(
        4,
        "A second device asked for the same username",
        &format!(
            "it became {}#{}, a different user from {}#{}",
            shared_name.username,
            shared_name.discriminator,
            identity.username,
            identity.discriminator
        ),
    );

    // 4. A challenge is spent once, so a captured signature buys nothing.
    let replayed = ada.authenticate(&ada_device).await;
    step(
        5,
        "Replaying on the same connection is refused",
        &describe(&replayed),
    );

    // 5. A fresh connection gets a fresh challenge, and the device is known.
    ada.shutdown().await;
    let returning = NexusClient::connect(&endpoint).await?;
    let again = returning.authenticate(&ada_device).await?;
    step(
        6,
        "The same device authenticated on a new connection",
        &format!(
            "still {}#{}, user {}",
            again.username,
            again.discriminator,
            short(&again.user_id)
        ),
    );

    // 6. A device the server has never seen cannot authenticate.
    let stranger = NexusClient::connect(&endpoint).await?;
    let refused = stranger.authenticate(&DemoDevice::ephemeral(11)).await;
    step(7, "An unenrolled device is refused", &describe(&refused));

    // 7. Ping still works, and is correlated to its request.
    let pong = returning.ping(42).await?;
    step(
        8,
        "Ping is answered with a correlated pong",
        &format!("correlated to {}", short(&pong.correlation_id)),
    );

    // 8. Encrypted shares, end to end: publish, refuse a stale revision,
    //    keep it private, then grant and hand over the key.
    shares(
        &returning,
        &ada_device,
        &grace,
        &grace_device,
        &shared_name.user_id,
    )
    .await?;

    // 9. Swarm discovery over the addresses the sockets observed.
    swarm(&returning, &grace).await?;

    // 15. Draining closes every live socket within a bounded wait.
    let live = [returning, grace, stranger];
    timeout(GRACEFUL_DRAIN_TIMEOUT, state.shutdown().drain()).await?;
    step(
        16,
        "The server drained",
        &format!("{} connections were asked to close", live.len()),
    );
    for client in live {
        client.shutdown().await;
    }
    server.abort();

    println!("\nIdentities lived in memory, so they are gone with this process.");
    println!("Run `cargo run -p portalis-nexus-demo --bin client` against a");
    println!("separately started server to see a device persist its key.");
    Ok(())
}

/// Publishing an encrypted share, keeping it private, and handing its key to
/// one other user's device.
///
/// The point is what Nexus never holds: it stores an opaque capsule, decides
/// who may fetch it, and relays a sealed key it cannot open.
async fn shares(
    ada: &NexusClient,
    ada_device: &DemoDevice,
    grace: &NexusClient,
    grace_device: &DemoDevice,
    grace_id: &[u8],
) -> Result<(), Box<dyn Error>> {
    let share_id = portalis_nexus_protocol::new_message_id();
    let share_key = portalis_nexus_protocol::new_challenge();
    let capsule = pretend_to_encrypt(MAGNET, &share_key);
    let snapshot_id = *blake3::hash(&capsule).as_bytes();
    let context = portalis_nexus_protocol::EnvelopeContext {
        share_id: share_id.as_slice().try_into()?,
        recipient_device_id: grace_device.device_id(),
    };

    let published = ada
        .publish_share(
            &share_id,
            1,
            None,
            &snapshot_id,
            &capsule,
            &ada_device.sign(&capsule),
        )
        .await?;
    step(
        9,
        "Ada published an encrypted share",
        &format!(
            "share {} at revision {}, a {}-byte capsule Nexus cannot read",
            short(&published.share_id),
            published.revision,
            published.capsule.len()
        ),
    );

    // A revision built on a snapshot the share has moved past is refused,
    // so two devices publishing at once cannot overwrite each other.
    let replaced = ada
        .publish_share(
            &share_id,
            2,
            Some(&[0; 32]),
            &snapshot_id,
            &capsule,
            &ada_device.sign(&capsule),
        )
        .await;
    step(
        10,
        "A publication built on a snapshot the share has moved past is refused",
        &describe(&replaced),
    );

    // A private share and one that was never published answer identically,
    // so an outsider cannot learn which identifiers exist.
    let probed = grace.fetch_share(&share_id).await;
    let invented = grace
        .fetch_share(&portalis_nexus_protocol::new_message_id())
        .await;
    step(
        11,
        "A private share and one that does not exist answer identically",
        &format!("{} / {}", describe(&probed), describe(&invented)),
    );

    // Ada grants Grace access and seals the share key to Grace's device.
    ada.grant_share_access(&share_id, grace_id).await?;
    let sealed =
        portalis_nexus_protocol::seal(&grace_device.encryption_public_key(), &context, &share_key)?;
    ada.put_key_envelope(
        &share_id,
        &grace_device.device_id(),
        &sealed.ephemeral_public_key,
        &sealed.ciphertext,
    )
    .await?;
    step(
        12,
        "Ada granted Grace access and sealed the share key to her device",
        &format!(
            "one envelope, {} bytes of ciphertext",
            sealed.ciphertext.len()
        ),
    );

    // Grace fetches the capsule, opens her envelope, and reads the descriptor
    // Nexus only ever saw encrypted.
    let fetched = grace.fetch_share(&share_id).await?;
    let page = grace.list_key_envelopes(None).await?;
    let envelope = page
        .envelopes
        .first()
        .ok_or("Grace should have exactly one envelope")?;
    let recovered = portalis_nexus_protocol::open(
        &grace_device.encryption_secret(),
        &context,
        &portalis_nexus_protocol::SealedEnvelope {
            ephemeral_public_key: envelope.ephemeral_public_key.as_slice().try_into()?,
            ciphertext: envelope.ciphertext.clone(),
        },
    )?;
    let opened = pretend_to_encrypt(&fetched.capsule, &recovered);
    step(
        13,
        "Grace opened her envelope and read the capsule",
        &format!(
            "revision {} decrypts to {:?}",
            fetched.revision,
            String::from_utf8_lossy(&opened)
        ),
    );
    Ok(())
}

/// Announcing to a swarm and finding peers at the addresses their sockets
/// were observed on, never one a client asked to advertise.
async fn swarm(ada: &NexusClient, grace: &NexusClient) -> Result<(), Box<dyn Error>> {
    let info_hash = [3_u8; 20];
    let lease = ada
        .announce_peer(&info_hash, 6881, AddressFamily::Ipv4, 1, 90)
        .await?;
    grace
        .announce_peer(&info_hash, 51413, AddressFamily::Ipv4, 1, 90)
        .await?;
    let found = ada
        .lookup_peers(&info_hash, AddressFamily::Unspecified, 0)
        .await?;
    step(
        14,
        "Two seeders announced and discovered each other",
        &format!(
            "lease until {}; Ada sees {} peer(s): {}",
            lease.expires_at_unix_ns,
            found.peers.len(),
            found
                .peers
                .iter()
                .map(|peer| format!("{}:{}", render_ip(&peer.ip_address), peer.port))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );

    // Withdrawing removes a lease before it expires.
    grace.withdraw_peer(&info_hash).await?;
    let after = ada
        .lookup_peers(&info_hash, AddressFamily::Unspecified, 0)
        .await?;
    step(
        15,
        "A withdrawn seeder disappears immediately",
        &format!("{} peer(s) remain", after.peers.len()),
    );
    Ok(())
}

/// Starts the real server on an ephemeral port.
///
/// The authority must match the address clients dial, because a signature is
/// bound to the server it was meant for.
async fn start_server() -> Result<(SocketAddr, AppState, JoinHandle<()>), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let state = AppState::default().with_server_authority(&address.to_string());
    state.mark_ready();
    // Hosting the router is ordinary axum, with one requirement: swarm
    // discovery binds a peer lease to the address the socket observed, so the
    // service has to carry connect info or every upgrade is refused.
    let router = portalis_nexus_server::app(&state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    Ok((address, state, handle))
}

/// Stands in for encrypting a capsule under a share key.
///
/// A real client uses an AEAD; the point being demonstrated is who holds the
/// key, not which cipher runs, and a self-inverse keeps the demo honest about
/// Nexus storing only bytes it cannot read. Applying it twice recovers the
/// plaintext, which is why decrypting calls the same function.
fn pretend_to_encrypt(plaintext: &[u8], key: &[u8]) -> Vec<u8> {
    plaintext
        .iter()
        .zip(key.iter().cycle())
        .map(|(byte, key_byte)| byte ^ key_byte)
        .collect()
}

/// Renders the address a lookup returned, in whichever family it arrived.
fn render_ip(bytes: &[u8]) -> String {
    match <[u8; 4]>::try_from(bytes) {
        Ok(octets) => std::net::Ipv4Addr::from(octets).to_string(),
        Err(_) => match <[u8; 16]>::try_from(bytes) {
            Ok(octets) => std::net::Ipv6Addr::from(octets).to_string(),
            Err(_) => "unknown".to_owned(),
        },
    }
}

/// Describes a refusal in one line, or says it unexpectedly succeeded.
fn describe<T>(outcome: &Result<T, TransportError>) -> String {
    match outcome {
        Ok(_) => "unexpectedly accepted".to_owned(),
        Err(TransportError::Client(ClientError::Refused { code, message })) => {
            format!("{code:?}: {message}")
        }
        Err(error) => format!("{error}"),
    }
}

fn step(number: usize, title: &str, detail: &str) {
    println!("\n{number}. {title}\n   {detail}");
}
