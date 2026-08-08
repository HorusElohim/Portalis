use std::net::{AddrParseError, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router, extract::State};
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, Ping, Pong, ProtocolError, ProtocolErrorCode, ServerHello,
};
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, new_challenge, new_message_id};
use portalis_nexus_server_core::ProtocolPolicy;
use serde::Serialize;
use tower_http::trace::TraceLayer;

mod socket;

pub const SERVICE_NAME: &str = "portalis-nexus";
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";
pub const SOCKET_PATH: &str = "/v1/socket";

#[derive(Clone, Debug)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    protocol_policy: ProtocolPolicy,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            protocol_policy: ProtocolPolicy::new(
                CURRENT_PROTOCOL_VERSION,
                CURRENT_PROTOCOL_VERSION,
            )
            .expect("the current protocol version is a valid range"),
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

    #[must_use]
    pub fn protocol_policy(&self) -> &ProtocolPolicy {
        &self.protocol_policy
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
        .route(SOCKET_PATH, get(socket::upgrade))
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

#[must_use]
pub fn server_hello(protocol_policy: &ProtocolPolicy, server_time_unix_ms: u64) -> Envelope {
    Envelope {
        message_id: new_message_id(),
        correlation_id: Vec::new(),
        sent_at_unix_ms: server_time_unix_ms,
        payload: Some(Payload::ServerHello(hello_payload(
            protocol_policy,
            server_time_unix_ms,
        ))),
    }
}

#[must_use]
pub fn hello_payload(protocol_policy: &ProtocolPolicy, server_time_unix_ms: u64) -> ServerHello {
    ServerHello {
        connection_id: new_message_id(),
        challenge: new_challenge(),
        server_time_unix_ms,
        supported_protocols: Some(*protocol_policy.supported()),
    }
}

#[must_use]
pub fn response_for(envelope: &Envelope, sent_at_unix_ms: u64) -> Envelope {
    match &envelope.payload {
        Some(Payload::Ping(Ping { nonce })) => Envelope {
            message_id: new_message_id(),
            correlation_id: envelope.message_id.clone(),
            sent_at_unix_ms,
            payload: Some(Payload::Pong(Pong { nonce: *nonce })),
        },
        _ => protocol_error(
            envelope.message_id.clone(),
            "only Ping is accepted before authentication".to_owned(),
            sent_at_unix_ms,
        ),
    }
}

#[must_use]
pub fn protocol_error(correlation_id: Vec<u8>, message: String, sent_at_unix_ms: u64) -> Envelope {
    Envelope {
        message_id: new_message_id(),
        correlation_id,
        sent_at_unix_ms,
        payload: Some(Payload::ProtocolError(ProtocolError {
            code: ProtocolErrorCode::InvalidMessage as i32,
            message,
            retry_after_ms: None,
            retryable: false,
        })),
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

    #[test]
    fn server_hello_is_valid_for_the_current_protocol() {
        let state = AppState::default();
        let hello = hello_payload(state.protocol_policy(), 42);

        assert_eq!(hello.server_time_unix_ms, 42);
        assert_eq!(
            portalis_nexus_protocol::validate_server_hello(&hello),
            Ok(())
        );
    }

    #[test]
    fn responds_to_ping_and_rejects_other_messages() {
        let request = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        };
        let response = response_for(&request, 2);
        assert_eq!(response.correlation_id, request.message_id);
        assert_eq!(response.sent_at_unix_ms, 2);
        assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 7 })));

        let rejected = response_for(&response, 3);
        assert_eq!(rejected.correlation_id, response.message_id);
        assert_eq!(
            rejected.payload,
            Some(Payload::ProtocolError(ProtocolError {
                code: ProtocolErrorCode::InvalidMessage as i32,
                message: "only Ping is accepted before authentication".to_owned(),
                retry_after_ms: None,
                retryable: false,
            }))
        );
    }
}
