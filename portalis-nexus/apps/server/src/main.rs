use std::error::Error;

use portalis_nexus_server::{AppState, GRACEFUL_DRAIN_TIMEOUT, ServerConfig, app};
use tokio::time::timeout;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let listen_value = std::env::var("PORTALIS_NEXUS_LISTEN_ADDR").ok();
    let config = ServerConfig::from_listen_value(listen_value.as_deref())?;
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let state = AppState::default();
    state.mark_ready();

    info!(listen_addr = %config.listen_addr, "Portalis Nexus is ready");
    axum::serve(listener, app(&state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Upgraded sockets outlive the HTTP serve loop, so drain them explicitly.
    info!("draining live Nexus sockets");
    if timeout(GRACEFUL_DRAIN_TIMEOUT, state.shutdown().drain())
        .await
        .is_err()
    {
        warn!(
            timeout_secs = GRACEFUL_DRAIN_TIMEOUT.as_secs(),
            "drain timed out; closing remaining sockets"
        );
    }
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal handler");
    }
}
