//! Liveness and readiness endpoints used by orchestrators.

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
use serde::Serialize;

use crate::state::AppState;

pub const SERVICE_NAME: &str = "portalis-nexus";

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    service: &'static str,
    status: &'static str,
    protocol_version: u32,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
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
    use crate::app;

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
