use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{extract::State, response::IntoResponse};
use futures_util::StreamExt;
use portalis_nexus_protocol::{MAX_FRAME_BYTES, WEBSOCKET_SUBPROTOCOL, decode_frame, encode_frame};
use portalis_nexus_server_core::ProtocolPolicy;

use crate::{AppState, protocol_error, response_for, server_hello};

pub(crate) async fn upgrade(
    websocket: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let protocol_policy = state.protocol_policy().clone();
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .max_frame_size(MAX_FRAME_BYTES)
        .max_message_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, protocol_policy))
}

async fn handle_socket(mut socket: WebSocket, protocol_policy: ProtocolPolicy) {
    let hello = server_hello(&protocol_policy, now_unix_ms());
    if send_envelope(&mut socket, &hello).await.is_err() {
        return;
    }

    while let Some(message) = socket.next().await {
        let Ok(message) = message else {
            return;
        };
        match message {
            Message::Binary(frame) => {
                let response = match decode_frame(&frame) {
                    Ok(envelope) => response_for(&envelope, now_unix_ms()),
                    Err(error) => protocol_error(Vec::new(), error.to_string(), now_unix_ms()),
                };
                if send_envelope(&mut socket, &response).await.is_err() {
                    return;
                }
            }
            Message::Ping(payload) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    return;
                }
            }
            Message::Text(_) => {
                let response = protocol_error(
                    Vec::new(),
                    "expected a binary protobuf envelope".to_owned(),
                    now_unix_ms(),
                );
                if send_envelope(&mut socket, &response).await.is_err() {
                    return;
                }
            }
            Message::Close(_) => return,
            Message::Pong(_) => {}
        }
    }
}

async fn send_envelope(
    socket: &mut WebSocket,
    envelope: &portalis_nexus_protocol::v1::Envelope,
) -> Result<(), axum::Error> {
    let frame = encode_frame(envelope).expect("server-generated envelopes are valid and bounded");
    socket.send(Message::Binary(frame.into())).await
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_millis(),
    )
    .expect("milliseconds since the Unix epoch fit in u64")
}
