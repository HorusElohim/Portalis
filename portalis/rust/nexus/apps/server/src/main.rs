use std::error::Error;

use portalis_nexus_server::{
    AppState, GRACEFUL_DRAIN_TIMEOUT, MongoStore, NexusStore, ServerConfig, app,
};
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

    let config = ServerConfig::from_environment()?;
    let uri = config.require_mongodb_uri()?;
    info!(database = %config.database, "connecting to MongoDB");
    let store = NexusStore::mongo(MongoStore::connect(uri, &config.database).await?);
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    // Clients sign against the name they dialled, so a deployment reached by
    // any other name refuses every signature. Logged for exactly that reason.
    let state = AppState::with_store(store).with_server_authority(&config.server_authority);
    // Ready only once the store is reachable and its indexes exist.
    state.mark_ready();

    info!(
        listen_addr = %config.listen_addr,
        server_authority = %config.server_authority,
        store = state.store().kind(),
        "Portalis Nexus is ready"
    );
    axum::serve(
        listener,
        app(&state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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

/// Waits for the first shutdown signal this platform can deliver.
///
/// Container runtimes and init systems ask a process to stop with `SIGTERM`;
/// only a terminal sends `SIGINT`. Listening for the interrupt alone means an
/// orchestrator's polite stop is ignored until its grace period expires and
/// the process is killed, severing live sockets rather than draining them.
///
/// A handler that cannot be installed waits forever instead of returning:
/// reporting "shut down now" because the listener failed would turn a missing
/// signal handler into an immediate shutdown.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for an interrupt");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot listen for a termination request");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => info!("interrupted; shutting down"),
        () = terminate => info!("termination requested; shutting down"),
    }
}
