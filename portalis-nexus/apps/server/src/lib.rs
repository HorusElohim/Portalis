//! The Portalis Nexus control-plane server.
//!
//! Module layout:
//!
//! - [`config`]: process configuration read at startup.
//! - [`state`]: state shared by every handler.
//! - [`shutdown`]: graceful draining of upgraded sockets.
//! - [`health`]: liveness and readiness endpoints.
//! - [`messages`]: envelope construction and inbound dispatch decisions.
//! - `socket`: the WebSocket plumbing those decisions drive.

use axum::Router;
use axum::routing::get;
use tower_http::trace::TraceLayer;

mod config;
mod health;
mod messages;
mod shutdown;
mod socket;
mod state;

pub use config::{DEFAULT_LISTEN_ADDR, ServerConfig};
pub use health::SERVICE_NAME;
pub use messages::{
    SocketReply, binary_frame, hello_envelope, hello_payload, protocol_error, reply_to,
    response_for, server_hello,
};
pub use shutdown::{GRACEFUL_DRAIN_TIMEOUT, Shutdown};
pub use state::AppState;

pub const SOCKET_PATH: &str = "/v1/socket";

pub fn app(state: &AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .route(SOCKET_PATH, get(socket::upgrade))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone())
}
