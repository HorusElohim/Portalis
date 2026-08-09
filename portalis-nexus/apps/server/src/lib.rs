//! The Portalis Nexus control-plane server.
//!
//! Module layout:
//!
//! - [`config`]: process configuration read at startup.
//! - [`state`]: state shared by every handler.
//! - [`shutdown`]: graceful draining of upgraded sockets.
//! - [`health`]: liveness and readiness endpoints.
//! - [`connections`]: where to reach a live connection.
//! - [`environment`]: the production clock and random source.
//! - [`identity`]: the concrete services this server runs.
//! - [`store`]: the store behind them, in memory or durable.
//! - [`mongo`]: the durable adapter.
//! - [`messages`]: envelope construction and inbound dispatch decisions.
//! - [`session`]: per-connection authentication state.
//! - [`handlers`]: domain commands, one module per subsystem.
//! - `socket`: the WebSocket plumbing those decisions drive.

use axum::Router;
use axum::routing::get;
use tower_http::trace::TraceLayer;

mod config;
mod connections;
mod environment;
mod handlers;
mod health;
mod identity;
mod messages;
mod mongo;
mod session;
mod shutdown;
mod socket;
mod state;
mod store;

pub use config::{DEFAULT_DATABASE, DEFAULT_LISTEN_ADDR, MissingMongoUri, ServerConfig};
pub use connections::Connections;
pub use environment::{OsRandom, SystemClock, now_unix_ms};
pub use handlers::{departed, dispatch};
pub use health::SERVICE_NAME;
pub use identity::{DefaultStore, NexusFriends, NexusIdentities, friends, identities};
pub use messages::{
    SocketReply, authenticated_reply, binary_frame, hello_envelope, hello_payload, presence_event,
    protocol_error, reply_to, reply_with, response_for, server_hello,
};
pub use mongo::MongoStore;
pub use session::Session;
pub use shutdown::{GRACEFUL_DRAIN_TIMEOUT, Shutdown};
pub use state::AppState;
pub use store::NexusStore;

pub const SOCKET_PATH: &str = "/v1/socket";

pub fn app(state: &AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .route(SOCKET_PATH, get(socket::upgrade))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone())
}
