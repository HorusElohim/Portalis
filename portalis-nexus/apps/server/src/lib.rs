use std::net::{AddrParseError, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router, extract::State};
use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
use serde::Serialize;
use tower_http::trace::TraceLayer;

pub const SERVICE_NAME: &str = "portalis-nexus";
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";

#[derive(Clone, Debug)]
pub struct AppState {
    ready: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AppState {
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
}

impl ServerConfig {
    /// Parses the configured address or applies the local development default.
    ///
    /// # Errors
    ///
    /// Returns [`AddrParseError`] when the supplied address is malformed.
    pub fn from_listen_value(value: Option<&str>) -> Result<Self, AddrParseError> {
        let listen_addr = value.unwrap_or(DEFAULT_LISTEN_ADDR).parse()?;
        Ok(Self { listen_addr })
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    protocol_version: u32,
}

pub fn app(state: &AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone())
}

#[allow(clippy::unused_async)]
async fn live() -> Json<HealthResponse> {
    Json(health("ok"))
}

#[allow(clippy::unused_async)]
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    if state.is_ready() {
        (StatusCode::OK, Json(health("ready")))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(health("not_ready")))
    }
}

fn health(status: &'static str) -> HealthResponse {
    HealthResponse {
        service: SERVICE_NAME,
        status,
        protocol_version: CURRENT_PROTOCOL_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    async fn request(state: AppState, path: &str) -> (StatusCode, Value) {
        let response = app(&state)
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096)
            .await
            .expect("bounded response body");
        let value = serde_json::from_slice(&body).expect("valid health JSON");
        (status, value)
    }

    #[test]
    fn config_uses_default_address() {
        let config = ServerConfig::from_listen_value(None).expect("valid default address");

        assert_eq!(config.listen_addr.to_string(), DEFAULT_LISTEN_ADDR);
    }

    #[test]
    fn config_accepts_custom_address() {
        let config =
            ServerConfig::from_listen_value(Some("0.0.0.0:9000")).expect("valid custom address");

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:9000");
    }

    #[test]
    fn config_rejects_invalid_address() {
        assert!(ServerConfig::from_listen_value(Some("not-an-address")).is_err());
    }

    #[tokio::test]
    async fn liveness_is_always_available() {
        let (status, body) = request(AppState::default(), "/health/live").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({
                "service": SERVICE_NAME,
                "status": "ok",
                "protocol_version": CURRENT_PROTOCOL_VERSION,
            })
        );
    }

    #[tokio::test]
    async fn readiness_is_unavailable_before_startup_finishes() {
        let (status, body) = request(AppState::default(), "/health/ready").await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "not_ready");
    }

    #[tokio::test]
    async fn readiness_becomes_available() {
        let state = AppState::default();
        state.mark_ready();
        let (status, body) = request(state, "/health/ready").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ready");
    }
}
