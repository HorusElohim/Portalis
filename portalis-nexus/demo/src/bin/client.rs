//! A client that keeps its device key between runs.
//!
//! Start the server first:
//!
//! ```text
//! cargo run -p portalis-nexus-server
//! ```
//!
//! Then run this once to register, and again to authenticate with the key it
//! saved. That is the shape a real application follows: the keypair is durable,
//! registration happens once, and every later connection just proves ownership.

use std::error::Error;
use std::path::PathBuf;

use portalis_nexus_client::{ClientError, NexusClient, TransportError, authority_of};
use portalis_nexus_demo::{DemoDevice, short};

/// Where this demo keeps its device key. A real app would use a keychain.
const DEFAULT_KEY_PATH: &str = "demo-device.key";
const DEFAULT_ENDPOINT: &str = "ws://127.0.0.1:8080/v1/socket";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let endpoint = arguments
        .next()
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned());
    let username = arguments.next().unwrap_or_else(|| "ada".to_owned());
    let key_path = PathBuf::from(
        std::env::var("PORTALIS_NEXUS_DEMO_KEY").unwrap_or_else(|_| DEFAULT_KEY_PATH.to_owned()),
    );

    let (device, created) = DemoDevice::load_or_create(&key_path)?;
    println!(
        "device {} ({} {})",
        short(&device.device_id()),
        if created { "created at" } else { "loaded from" },
        key_path.display()
    );

    // The signature is bound to the authority dialled, so both sides must
    // name this server the same way.
    println!(
        "connecting to {endpoint} as authority {}",
        authority_of(&endpoint)
    );
    let client = NexusClient::connect(&endpoint).await?;

    // Authenticate first; a device the server does not know registers instead.
    // Each connection may sign once, so the attempt spends this one either way.
    let attempt = client.authenticate(&device).await;
    client.shutdown().await;

    let identity = match attempt {
        Ok(identity) => {
            println!("authenticated an already-enrolled device");
            identity
        }
        Err(TransportError::Client(ClientError::Refused { code, message })) => {
            println!("not enrolled yet ({code:?}: {message}); registering");
            let fresh = NexusClient::connect(&endpoint).await?;
            let identity = fresh.register(&username, &device).await?;
            fresh.shutdown().await;
            identity
        }
        Err(error) => return Err(error.into()),
    };

    println!(
        "you are {}#{} (user {})",
        identity.username,
        identity.discriminator,
        short(&identity.user_id)
    );
    println!("run this again to authenticate with the same key");
    Ok(())
}
