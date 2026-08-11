//! Standalone in-memory Nexus server for the two-process M6 demo.

use std::error::Error;
use std::net::SocketAddr;

use portalis_nexus_server::{AppState, GRACEFUL_DRAIN_TIMEOUT};
use tokio::time::timeout;

const DEFAULT_ADDRESS: &str = "127.0.0.1:8090";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address: SocketAddr = std::env::var("PORTALIS_NEXUS_M6_ADDRESS")
        .unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    let state = AppState::default().with_server_authority(&address.to_string());
    state.mark_ready();

    println!("Nexus M6 demo server is ready at ws://{address}/v1/socket");
    println!("In another terminal run:");
    println!("  cargo run -p portalis-nexus-m6-demo --bin nexus-demo-client");
    println!("Press Ctrl-C to drain the server.");

    let shutdown_state = state.clone();
    axum::serve(
        listener,
        portalis_nexus_server::app(&state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = timeout(GRACEFUL_DRAIN_TIMEOUT, shutdown_state.shutdown().drain()).await;
    })
    .await?;
    Ok(())
}
