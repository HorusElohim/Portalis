//! M4 encrypted-share privacy and monotonic publication over real sockets.

use portalis_nexus_client::{ClientError, DeviceSigner, NexusClient, TransportError};
use portalis_nexus_protocol::v1::ProtocolErrorCode;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::{
    EnvelopeContext, SIGNATURE_BYTES, SealedEnvelope, derive_device_id, open, seal,
};

mod common;

use common::{device, endpoint, reserve_address, start_server};

const SHARE: [u8; 16] = [42; 16];
const FIRST: [u8; 32] = [1; 32];
const SECOND: [u8; 32] = [2; 32];
const SIGNATURE: [u8; SIGNATURE_BYTES] = [9; SIGNATURE_BYTES];

fn refusal(error: &TransportError) -> ProtocolErrorCode {
    let TransportError::Client(ClientError::Refused { code, .. }) = error else {
        panic!("expected a typed refusal, got {error:?}");
    };
    *code
}

async fn expect_handoff(
    owner: &NexusClient,
    events: &mut tokio::sync::mpsc::Receiver<portalis_nexus_protocol::v1::Envelope>,
    recipient_device_id: &[u8; 32],
) {
    owner
        .share_handoff(&SHARE, recipient_device_id, b"live encrypted torrent")
        .await
        .expect("authorized live handoff");
    let delivered = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let event = events.recv().await.expect("event stream stays live");
            if let Some(Payload::ShareHandoff(handoff)) = event.payload {
                break handoff;
            }
        }
    })
    .await
    .expect("handoff arrives");
    assert_eq!(delivered.recipient_device_id, recipient_device_id);
    assert_eq!(delivered.ciphertext, b"live encrypted torrent");
}

#[tokio::test]
async fn only_an_authorized_user_discovers_the_latest_encrypted_snapshot() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let owner_device = device(31);
    let member_device = device(32);
    let owner = NexusClient::connect(&endpoint(address))
        .await
        .expect("owner connects");
    let member = NexusClient::connect(&endpoint(address))
        .await
        .expect("member connects");
    let mut events = member.events().expect("member event stream");
    owner
        .register("Ada", &owner_device)
        .await
        .expect("owner registers");
    let member_identity = member
        .register("Grace", &member_device)
        .await
        .expect("member registers");

    owner
        .publish_share(&SHARE, 1, None, &FIRST, b"encrypted one", &SIGNATURE)
        .await
        .expect("first revision");

    let hidden = member
        .fetch_share(&SHARE)
        .await
        .expect_err("private share is hidden");
    assert_eq!(refusal(&hidden), ProtocolErrorCode::NotFound);
    assert!(member.list_shares().await.expect("list").is_empty());

    owner
        .grant_share_access(&SHARE, &member_identity.user_id)
        .await
        .expect("access granted");
    let member_device_id = derive_device_id(&member_device.public_key());
    let context = EnvelopeContext {
        share_id: SHARE,
        recipient_device_id: member_device_id,
    };
    let sealed = seal(
        &member_device.encryption_public_key(),
        &context,
        b"share secret",
    )
    .expect("sealed for the member device");
    owner
        .put_key_envelope(
            &SHARE,
            &member_device_id,
            &sealed.ephemeral_public_key,
            &sealed.ciphertext,
        )
        .await
        .expect("member key envelope stored");
    owner
        .publish_share(
            &SHARE,
            2,
            Some(&FIRST),
            &SECOND,
            b"encrypted two",
            &SIGNATURE,
        )
        .await
        .expect("second revision");

    let fetched = member.fetch_share(&SHARE).await.expect("authorized fetch");
    assert_eq!(fetched.revision, 2);
    assert_eq!(fetched.snapshot_id, SECOND);
    assert_eq!(fetched.capsule, b"encrypted two");
    let envelope = member
        .list_key_envelopes(None)
        .await
        .expect("member envelope listed")
        .envelopes
        .into_iter()
        .next()
        .expect("one envelope");
    let key = open(
        &member_device.encryption_secret_key(),
        &context,
        &SealedEnvelope {
            ephemeral_public_key: envelope.ephemeral_public_key.try_into().expect("fixed key"),
            ciphertext: envelope.ciphertext,
        },
    )
    .expect("member decrypts its share key");
    assert_eq!(key, b"share secret");

    expect_handoff(&owner, &mut events, &member_device_id).await;

    let regressed = owner
        .publish_share(&SHARE, 1, None, &FIRST, b"changed", &SIGNATURE)
        .await
        .expect_err("published history never regresses");
    assert_eq!(refusal(&regressed), ProtocolErrorCode::InvalidMessage);

    owner.shutdown().await;
    member.shutdown().await;
    server.abort();
}
