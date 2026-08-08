//! Registration and device authentication over a real socket.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::{ClientError, DeviceSigner, NexusClient, TransportError};
use portalis_nexus_protocol::v1::ProtocolErrorCode;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::{DEVICE_KEY_BYTES, SIGNATURE_BYTES, derive_device_id};

mod common;

use common::{endpoint, reserve_address, start_server};

/// A signer over a fixed key, so a test can re-create the same device.
struct TestDevice(SigningKey);

impl TestDevice {
    fn new(seed: u8) -> Self {
        Self(SigningKey::from_bytes(&[seed; 32]))
    }
}

impl DeviceSigner for TestDevice {
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.0.verifying_key().to_bytes()
    }

    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.0.sign(payload).to_bytes()
    }
}

/// Reads the code the server refused a request with.
fn refusal(error: &TransportError) -> ProtocolErrorCode {
    let TransportError::Client(ClientError::Refused { code, .. }) = error else {
        panic!("expected a typed refusal, got {error:?}");
    };
    *code
}

#[tokio::test]
async fn registers_a_user_and_returns_its_handle() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");
    let device = TestDevice::new(7);

    let identity = client
        .register("Ada", &device)
        .await
        .expect("registration succeeds");

    assert_eq!(identity.username, "Ada");
    assert_eq!(identity.discriminator.len(), 5);
    assert_eq!(identity.user_id.len(), 16);
    assert_eq!(identity.device_id, derive_device_id(&device.public_key()));
    assert_eq!(identity.protocol_version, 1);

    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn an_enrolled_device_authenticates_on_a_new_connection() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let device = TestDevice::new(7);
    let registrar = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect to register");
    let registered = registrar
        .register("Ada", &device)
        .await
        .expect("registration succeeds");
    registrar.shutdown().await;

    // A second connection gets its own challenge and must sign that one.
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect to authenticate");
    let identity = client
        .authenticate(&device)
        .await
        .expect("authentication succeeds");

    assert_eq!(identity.user_id, registered.user_id);
    assert_eq!(identity.device_id, registered.device_id);
    assert_eq!(identity.username, "Ada");
    assert_eq!(identity.discriminator, registered.discriminator);

    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn a_challenge_cannot_be_spent_twice() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");
    let device = TestDevice::new(7);
    client
        .register("Ada", &device)
        .await
        .expect("registration succeeds");

    // The same connection still holds a spent challenge.
    let error = client
        .authenticate(&device)
        .await
        .expect_err("the challenge was already used");

    assert_eq!(refusal(&error), ProtocolErrorCode::Unauthenticated);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn an_unknown_device_cannot_authenticate() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let error = client
        .authenticate(&TestDevice::new(11))
        .await
        .expect_err("the device was never enrolled");

    assert_eq!(refusal(&error), ProtocolErrorCode::Unauthenticated);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn a_username_that_breaks_the_rules_is_refused() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let error = client
        .register("ad", &TestDevice::new(7))
        .await
        .expect_err("the username is too short");

    assert_eq!(refusal(&error), ProtocolErrorCode::InvalidMessage);
    client.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn two_devices_sharing_a_username_get_different_handles() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;

    let first_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect first");
    let first = first_client
        .register("Ada", &TestDevice::new(7))
        .await
        .expect("first registration");
    first_client.shutdown().await;

    let second_client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect second");
    let second = second_client
        .register("Ada", &TestDevice::new(8))
        .await
        .expect("second registration");
    second_client.shutdown().await;

    assert_eq!(first.username, second.username);
    assert_ne!(first.discriminator, second.discriminator);
    assert_ne!(first.user_id, second.user_id);
    server.abort();
}

#[tokio::test]
async fn the_server_still_answers_pings_before_authentication() {
    let address = reserve_address().await;
    let (_state, server) = start_server(address).await;
    let client = NexusClient::connect(&endpoint(address))
        .await
        .expect("connect Nexus client");

    let response = client.ping(9).await.expect("pong");

    assert!(matches!(response.payload, Some(Payload::Pong(_))));
    client.shutdown().await;
    server.abort();
}
