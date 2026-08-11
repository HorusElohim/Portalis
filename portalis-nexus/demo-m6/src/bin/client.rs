//! Two concurrent devices exercising the M6 Nexus path against a standalone server.

use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use portalis_nexus_client::{
    CapsuleContext, DeviceSigner, HandoffContext, Manifest, ManifestEntry, NexusClient,
    SHARE_KEY_BYTES, open_capsule, open_handoff, seal_capsule, seal_handoff,
};
use portalis_nexus_demo::{DemoDevice, init_tracing, short};
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{AddressFamily, FriendAction, FriendshipState, ShareHandoff};
use portalis_nexus_protocol::{EnvelopeContext, SealedEnvelope, new_challenge, new_message_id};
use tokio::sync::mpsc;
use tokio::time::timeout;

const DEFAULT_ENDPOINT: &str = "ws://127.0.0.1:8090/v1/socket";
const INFO_HASH: [u8; 20] = [0x42; 20];
const TORRENT_BYTES: &[u8] = b"d4:infod6:lengthi5e4:name5:hello12:piece lengthi16384e6:pieces20:000000000000000000007:privatei1eee";

#[tokio::main]
// This is a narrated executable: keeping the ten wire steps in one function
// makes the terminal output and the protocol order readable side by side.
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing("portalis_nexus_client=debug");
    let endpoint =
        std::env::var("PORTALIS_NEXUS_M6_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
    println!("Connecting Alice and Bob concurrently to {endpoint}");

    let alice_device = DemoDevice::ephemeral(21);
    let bob_device = DemoDevice::ephemeral(22);
    let (alice, bob) = tokio::try_join!(
        NexusClient::connect(&endpoint),
        NexusClient::connect(&endpoint)
    )?;
    let mut alice_events = alice
        .events()
        .ok_or("Alice's event stream is unavailable")?;
    let bob_events = bob.events().ok_or("Bob's event stream is unavailable")?;

    let (alice_identity, bob_identity) = tokio::try_join!(
        alice.register("alice", &alice_device),
        bob.register("bob", &bob_device)
    )?;
    step(
        1,
        "Two devices registered over independent live connections",
        &format!(
            "Alice {} / Bob {}",
            short(&alice_identity.device_id),
            short(&bob_identity.device_id)
        ),
    );

    tokio::try_join!(alice.ping(1), bob.ping(2))?;
    step(
        2,
        "Both connections answered correlated pings",
        "socket supervision is live",
    );

    let bob_handle = format!("{}#{}", bob_identity.username, bob_identity.discriminator);
    let resolved = alice.resolve_handle(&bob_handle).await?;
    alice
        .friend_command(FriendAction::Request, &resolved.user_id)
        .await?;
    bob.friend_command(FriendAction::Accept, &alice_identity.user_id)
        .await?;
    let friends = alice.list_friends().await?;
    if friends
        .iter()
        .all(|friend| friend.state != FriendshipState::Accepted as i32)
    {
        return Err("Alice and Bob did not become accepted friends".into());
    }
    step(3, "Handle resolution and friendship completed", &bob_handle);

    // Closing Bob proves that presence follows real sockets. Reconnecting and
    // authenticating the same device brings him back without registering again.
    drop(bob_events);
    bob.shutdown().await;
    wait_for_presence(&mut alice_events, &bob_identity.user_id, false).await?;
    step(
        4,
        "Alice observed Bob's connection go offline",
        "presence is derived, not stored",
    );

    let bob = NexusClient::connect(&endpoint).await?;
    let mut bob_events = bob.events().ok_or("Bob's event stream is unavailable")?;
    bob.authenticate(&bob_device).await?;
    wait_for_presence(&mut alice_events, &bob_identity.user_id, true).await?;
    step(
        5,
        "Bob re-authenticated",
        "Alice observed the same device come online",
    );

    let share_id: [u8; 16] = new_message_id()
        .try_into()
        .map_err(|_| "a generated share ID has the wrong length")?;
    let share_key: [u8; SHARE_KEY_BYTES] = new_challenge()
        .try_into()
        .map_err(|_| "a generated share key has the wrong length")?;
    let mut entry = ManifestEntry {
        info_hash: INFO_HASH,
        name: "hello.txt".to_owned(),
        thumbnail_hash: None,
        author_public_key: alice_device.public_key(),
        added_at_unix_ns: now_unix_ns(),
        signature: [0; 64],
    };
    entry.signature = alice_device.sign(&entry.signing_payload());
    let manifest = Manifest::new(vec![entry])?;
    let snapshot_id = manifest.snapshot_id();
    let capsule = seal_capsule(&share_key, share_id, 1, &manifest)?;
    let published = alice
        .publish_share(
            &share_id,
            1,
            None,
            &snapshot_id,
            &capsule,
            &alice_device.sign(&capsule),
        )
        .await?;
    step(
        6,
        "Alice published canonical revision one",
        &format!(
            "share {}, snapshot {}",
            short(&share_id),
            short(&published.snapshot_id)
        ),
    );

    let granted = alice
        .grant_share_access(&share_id, &bob_identity.user_id)
        .await?;
    let recipient = granted
        .recipient_devices
        .iter()
        .find(|device| device.device_id == bob_identity.device_id)
        .ok_or("the grant did not return Bob's device")?;
    let recipient_device_id: [u8; 32] = recipient.device_id.as_slice().try_into()?;
    let recipient_public_key: [u8; 32] = recipient.encryption_public_key.as_slice().try_into()?;
    let envelope_context = EnvelopeContext {
        share_id,
        recipient_device_id,
    };
    let sealed_key =
        portalis_nexus_protocol::seal(&recipient_public_key, &envelope_context, &share_key)?;
    alice
        .put_key_envelope(
            &share_id,
            &recipient_device_id,
            &sealed_key.ephemeral_public_key,
            &sealed_key.ciphertext,
        )
        .await?;
    step(
        7,
        "Alice granted Bob and stored his sealed share key",
        &format!("recipient device {}", short(&recipient_device_id)),
    );

    let key_page = bob.list_key_envelopes(None).await?;
    let key_envelope = key_page
        .envelopes
        .iter()
        .find(|envelope| envelope.share_id == share_id)
        .ok_or("Bob did not receive the share-key envelope")?;
    let recovered_key: [u8; SHARE_KEY_BYTES] = portalis_nexus_protocol::open(
        &bob_device.encryption_secret(),
        &envelope_context,
        &SealedEnvelope {
            ephemeral_public_key: key_envelope.ephemeral_public_key.as_slice().try_into()?,
            ciphertext: key_envelope.ciphertext.clone(),
        },
    )?
    .try_into()
    .map_err(|_| "the recovered share key has the wrong length")?;
    let fetched = bob.fetch_share(&share_id).await?;
    let opened_manifest = open_capsule(
        &recovered_key,
        &CapsuleContext {
            share_id,
            revision: fetched.revision,
            snapshot_id: fetched.snapshot_id.as_slice().try_into()?,
        },
        &fetched.capsule,
    )?;
    if opened_manifest.snapshot_id() != snapshot_id {
        return Err("Bob opened a different manifest".into());
    }
    step(
        8,
        "Bob fetched and opened the encrypted capsule",
        "Nexus never received the key",
    );

    let handoff_context = HandoffContext {
        share_id,
        recipient_device_id,
        info_hash: INFO_HASH,
    };
    let ciphertext = seal_handoff(&share_key, &handoff_context, "Alice and Bob", TORRENT_BYTES)?;
    alice
        .share_handoff(&share_id, &recipient_device_id, &INFO_HASH, &ciphertext)
        .await?;
    let delivered = wait_for_handoff(&mut bob_events).await?;
    let opened = open_handoff(&recovered_key, &handoff_context, &delivered.ciphertext)?;
    if opened.info_hash != INFO_HASH || opened.torrent_bytes != TORRENT_BYTES {
        return Err("the delivered torrent handoff did not match".into());
    }
    step(
        9,
        "Bob received the encrypted .torrent on his exact device",
        &format!(
            "{} bytes, info hash {}",
            opened.torrent_bytes.len(),
            short(&opened.info_hash)
        ),
    );

    tokio::try_join!(
        alice.announce_peer(&INFO_HASH, 6881, AddressFamily::Ipv4, 1, 90),
        bob.announce_peer(&INFO_HASH, 51413, AddressFamily::Ipv4, 1, 90)
    )?;
    let peers = bob.lookup_peers(&INFO_HASH, AddressFamily::Ipv4, 0).await?;
    step(
        10,
        "Both devices joined the Nexus-discovered private swarm",
        &format!("Bob sees {} candidate(s)", peers.peers.len()),
    );

    alice.shutdown().await;
    bob.shutdown().await;
    println!("\nM6 connection demo complete.");
    Ok(())
}

async fn wait_for_presence(
    events: &mut mpsc::Receiver<portalis_nexus_protocol::v1::Envelope>,
    user_id: &[u8],
    online: bool,
) -> Result<(), Box<dyn Error>> {
    timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.ok_or("Alice's event stream ended")?;
            if let Some(Payload::PresenceEvent(presence)) = event.payload
                && presence.user_id == user_id
                && presence.online == online
            {
                return Ok::<_, Box<dyn Error>>(());
            }
        }
    })
    .await??;
    Ok(())
}

async fn wait_for_handoff(
    events: &mut mpsc::Receiver<portalis_nexus_protocol::v1::Envelope>,
) -> Result<ShareHandoff, Box<dyn Error>> {
    timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.ok_or("Bob's event stream ended")?;
            if let Some(Payload::ShareHandoff(handoff)) = event.payload {
                return Ok::<_, Box<dyn Error>>(handoff);
            }
        }
    })
    .await?
}

fn now_unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn step(number: usize, title: &str, detail: &str) {
    println!("\n{number}. {title}\n   {detail}");
}
