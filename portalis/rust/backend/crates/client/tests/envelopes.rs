//! Encrypted share-key delivery between one user's own devices.
//!
//! This is the M2.5 gate: a second approved device receives only its own
//! encrypted envelope, decrypts the same share capsule locally, and a revoked
//! device cannot receive a replacement envelope. The capsule stands in for
//! whatever M4 will publish; what matters here is that the key reaches the
//! new device without Nexus ever holding it in plaintext.

use portalis_nexus_client::{ClientError, DeviceSigner, NexusClient, TransportError};
use portalis_nexus_protocol::v1::ProtocolErrorCode;
use portalis_nexus_protocol::{EnvelopeContext, derive_device_id, open, seal};

mod common;

use common::{device, endpoint, reserve_address, start_server};

/// A share key the way a client would generate one: random bytes it alone
/// chooses, never derived from anything Nexus knows.
const SHARE_KEY: &[u8] = b"a random per-share symmetric key";
const SHARE_ID: [u8; 16] = [42; 16];

fn refusal(error: &TransportError) -> ProtocolErrorCode {
    let TransportError::Client(ClientError::Refused { code, .. }) = error else {
        panic!("expected a typed refusal, got {error:?}");
    };
    *code
}

fn context(device: &impl DeviceSigner) -> EnvelopeContext {
    EnvelopeContext {
        share_id: SHARE_ID,
        recipient_device_id: derive_device_id(&device.public_key()),
    }
}

#[tokio::test]
async fn a_linked_device_decrypts_a_share_key_nexus_never_saw() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let first = device(7);
    let second = device(8);

    // The first device registers and approves the second.
    let first_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect the first device");
    first_client
        .register("Ada", &first)
        .await
        .expect("registration succeeds");
    first_client
        .link_device(
            &second.public_key(),
            &second.encryption_public_key(),
            &first,
        )
        .await
        .expect("linking succeeds");

    // It seals the share key to the second device's encryption key and hands
    // Nexus only the ciphertext.
    let context = context(&second);
    let sealed = seal(&second.encryption_public_key(), &context, SHARE_KEY).expect("seals");
    let stored = first_client
        .put_key_envelope(
            &SHARE_ID,
            &derive_device_id(&second.public_key()),
            &sealed.ephemeral_public_key,
            &sealed.ciphertext,
        )
        .await
        .expect("the envelope is stored");
    assert_eq!(stored.share_id, SHARE_ID);
    first_client.shutdown().await;

    // The second device authenticates on its own connection and fetches it.
    let second_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect the second device");
    second_client
        .authenticate(&second)
        .await
        .expect("the linked device authenticates");
    let envelopes = second_client
        .list_key_envelopes(None)
        .await
        .expect("envelopes listed");

    assert_eq!(envelopes.envelopes.len(), 1);
    assert_eq!(envelopes.envelopes[0].share_id, SHARE_ID);
    assert_ne!(
        envelopes.envelopes[0].ciphertext, SHARE_KEY,
        "the key crossed the wire sealed, not in the clear"
    );

    // Only the second device's own secret opens it.
    let recovered = open(
        &second.encryption_secret_key(),
        &context,
        &portalis_nexus_protocol::SealedEnvelope {
            ephemeral_public_key: envelopes.envelopes[0]
                .ephemeral_public_key
                .as_slice()
                .try_into()
                .expect("32 bytes"),
            ciphertext: envelopes.envelopes[0].ciphertext.clone(),
        },
    )
    .expect("the linked device opens its own envelope");

    assert_eq!(recovered, SHARE_KEY);
    second_client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn an_envelope_is_only_delivered_to_the_device_it_names() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let first = device(7);
    let second = device(8);

    let first_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect the first device");
    first_client
        .register("Ada", &first)
        .await
        .expect("registration succeeds");
    first_client
        .link_device(
            &second.public_key(),
            &second.encryption_public_key(),
            &first,
        )
        .await
        .expect("linking succeeds");
    let sealed = seal(
        &second.encryption_public_key(),
        &context(&second),
        SHARE_KEY,
    )
    .expect("seals");
    first_client
        .put_key_envelope(
            &SHARE_ID,
            &derive_device_id(&second.public_key()),
            &sealed.ephemeral_public_key,
            &sealed.ciphertext,
        )
        .await
        .expect("stored");

    // The sender asks for its own envelopes and is told about none: the one
    // it just stored was addressed to the other device.
    let mine = first_client
        .list_key_envelopes(None)
        .await
        .expect("envelopes listed");

    assert!(mine.envelopes.is_empty());
    first_client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn a_revoked_device_cannot_receive_a_replacement_envelope() {
    let address = reserve_address().await;
    let (state, server) = start_server(address).await;
    let first = device(7);
    let second = device(8);

    let first_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect the first device");
    first_client
        .register("Ada", &first)
        .await
        .expect("registration succeeds");
    first_client
        .link_device(
            &second.public_key(),
            &second.encryption_public_key(),
            &first,
        )
        .await
        .expect("linking succeeds");

    // The second device is revoked, the way a user would after losing it.
    let second_device_id = derive_device_id(&second.public_key());
    state
        .identities()
        .revoke_device(second_device_id)
        .await
        .expect("revocation succeeds");

    // Rotating the share key now must not reach the revoked device.
    let sealed = seal(
        &second.encryption_public_key(),
        &context(&second),
        b"rotated key",
    )
    .expect("seals");
    let refused = first_client
        .put_key_envelope(
            &SHARE_ID,
            &second_device_id,
            &sealed.ephemeral_public_key,
            &sealed.ciphertext,
        )
        .await
        .expect_err("a revoked device receives no replacement envelope");

    assert_eq!(refusal(&refused), ProtocolErrorCode::Unauthorized);
    first_client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn an_envelope_cannot_be_addressed_to_another_users_device() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let ada = device(7);
    let grace = device(9);

    let grace_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Grace");
    grace_client
        .register("Grace", &grace)
        .await
        .expect("registration succeeds");
    grace_client.shutdown().await;

    let ada_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Ada");
    ada_client
        .register("Ada", &ada)
        .await
        .expect("registration succeeds");

    let sealed = seal(&grace.encryption_public_key(), &context(&grace), SHARE_KEY).expect("seals");
    let refused = ada_client
        .put_key_envelope(
            &SHARE_ID,
            &derive_device_id(&grace.public_key()),
            &sealed.ephemeral_public_key,
            &sealed.ciphertext,
        )
        .await
        .expect_err("a device belonging to someone else cannot be addressed");

    assert_eq!(refusal(&refused), ProtocolErrorCode::Unauthorized);
    ada_client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn key_envelopes_require_authentication() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let listed = client
        .list_key_envelopes(None)
        .await
        .expect_err("nobody has authenticated on this connection");
    assert_eq!(refusal(&listed), ProtocolErrorCode::Unauthenticated);

    let stored = client
        .put_key_envelope(&SHARE_ID, &[1; 32], &[2; 32], b"sealed")
        .await
        .expect_err("nobody has authenticated on this connection");
    assert_eq!(refusal(&stored), ProtocolErrorCode::Unauthenticated);

    client.shutdown().await;
    server.abort();
}
