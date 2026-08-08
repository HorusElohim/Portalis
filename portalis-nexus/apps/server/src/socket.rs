use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{extract::State, response::IntoResponse};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use portalis_nexus_protocol::{MAX_FRAME_BYTES, MAX_OUTBOUND_QUEUE, WEBSOCKET_SUBPROTOCOL};
use tokio::sync::{mpsc, watch};

use crate::{AppState, SocketReply, binary_frame, reply_to, server_hello};

pub(crate) async fn upgrade(
    websocket: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .max_frame_size(MAX_FRAME_BYTES)
        .max_message_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

/// Runs one socket as a bounded read loop feeding a single writer task.
///
/// The writer owns the sink, so every outbound message crosses one queue of at
/// most [`MAX_OUTBOUND_QUEUE`] entries. A peer that stops reading fills that
/// queue and loses its connection instead of growing server memory.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let mut draining = state.shutdown().register();
    let (sink, mut stream) = socket.split();
    let (outbound, inbox) = mpsc::channel(MAX_OUTBOUND_QUEUE);
    let writer = tokio::spawn(write_outbound(sink, inbox));

    let hello = binary_frame(&server_hello(state.protocol_policy(), now_unix_ms()));
    if outbound.send(hello).await.is_ok() {
        read_inbound(&mut stream, &outbound, &mut draining).await;
    }

    drop(outbound);
    let _ = writer.await;
}

/// Reads until the peer leaves, the queue fills, or the server starts draining.
async fn read_inbound(
    stream: &mut SplitStream<WebSocket>,
    outbound: &mpsc::Sender<Message>,
    draining: &mut watch::Receiver<bool>,
) {
    loop {
        let message = tokio::select! {
            _ = draining.changed() => return,
            message = stream.next() => message,
        };
        let Some(Ok(message)) = message else {
            return;
        };
        match reply_to(&message, now_unix_ms()) {
            SocketReply::Send(reply) => {
                if outbound.try_send(reply).is_err() {
                    return;
                }
            }
            SocketReply::Idle => {}
            SocketReply::Close => return,
        }
    }
}

/// Writes queued messages, then closes the socket once the queue is dropped.
async fn write_outbound(
    mut sink: SplitSink<WebSocket, Message>,
    mut inbox: mpsc::Receiver<Message>,
) {
    while let Some(message) = inbox.recv().await {
        if sink.send(message).await.is_err() {
            return;
        }
    }
    let _ = sink.send(Message::Close(None)).await;
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
