//! A guided tour of Portalis Nexus, server and clients in one process.
//!
//! Run it with `cargo run -p portalis-nexus-demo`. Nothing is mocked: this is
//! the real server, the real portable client, and real sockets between them.

use std::error::Error;
use std::net::SocketAddr;

use portalis_nexus_client::{ClientError, NexusClient, TransportError};
use portalis_nexus_demo::{DemoDevice, short};
use portalis_nexus_server::{AppState, GRACEFUL_DRAIN_TIMEOUT};
use tokio::task::JoinHandle;
use tokio::time::timeout;

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

    // 8. Draining closes every live socket within a bounded wait.
    let live = [returning, grace, stranger];
    timeout(GRACEFUL_DRAIN_TIMEOUT, state.shutdown().drain()).await?;
    step(
        9,
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

/// Starts the real server on an ephemeral port.
///
/// The authority must match the address clients dial, because a signature is
/// bound to the server it was meant for.
async fn start_server() -> Result<(SocketAddr, AppState, JoinHandle<()>), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let state = AppState::default().with_server_authority(&address.to_string());
    state.mark_ready();
    // Hosting the router is ordinary axum; the control plane adds nothing to
    // it beyond the routes themselves.
    let router = portalis_nexus_server::app(&state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((address, state, handle))
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
