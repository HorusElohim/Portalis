//! WebSocket plumbing: one bounded read loop feeding one writer task.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{extract::State, response::IntoResponse};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use portalis_nexus_protocol::v1::ProtocolErrorCode;
use portalis_nexus_protocol::{
    MAX_FRAME_BYTES, MAX_OUTBOUND_QUEUE, WEBSOCKET_SUBPROTOCOL, decode_frame, format_id,
};
use tokio::sync::{mpsc, watch};
use tracing::{Instrument, debug, info_span, warn};

use crate::handlers::{departed, dispatch};
use crate::messages::{
    SocketReply, binary_frame, hello_envelope, hello_payload, protocol_error, reply_to,
};
use crate::session::Session;
use crate::state::AppState;

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
    let issued_at = now_unix_ms();
    let hello = hello_payload(state.protocol_policy(), issued_at);
    let span = info_span!(
        "nexus_socket",
        connection_id = %format_id(&hello.connection_id)
    );

    async move {
        let mut draining = state.shutdown().register();
        let (sink, mut stream) = socket.split();
        let (outbound, inbox) = mpsc::channel(MAX_OUTBOUND_QUEUE);
        let writer = tokio::spawn(write_outbound(sink, inbox));

        let mut session = Session::new(&hello);
        // Published before the greeting, so an event triggered by this
        // connection's own first command can already reach it.
        state
            .connections()
            .register(session.connection_id(), outbound.clone());

        let greeting = binary_frame(&hello_envelope(hello, issued_at));
        if outbound.send(greeting).await.is_ok() {
            debug!("socket established");
            read_inbound(&mut stream, &outbound, &mut draining, &mut session, &state).await;
        }

        departed(&session, &state, now_unix_ms()).await;
        drop(outbound);
        let _ = writer.await;
        debug!("socket closed");
    }
    .instrument(span)
    .await;
}

/// Reads until the peer leaves, the queue fills, or the server starts draining.
async fn read_inbound(
    stream: &mut SplitStream<WebSocket>,
    outbound: &mpsc::Sender<Message>,
    draining: &mut watch::Receiver<bool>,
    session: &mut Session,
    state: &AppState,
) {
    loop {
        let message = tokio::select! {
            _ = draining.changed() => {
                debug!("server is draining");
                return;
            }
            message = stream.next() => message,
        };
        let Some(Ok(message)) = message else {
            return;
        };
        let reply = match message {
            // Identity commands need the connection's own challenge state, so
            // they are answered here rather than by the stateless mapping.
            Message::Binary(ref frame) => match decode_frame(frame) {
                Ok(request) => SocketReply::Send(binary_frame(
                    &dispatch(session, state, &request, now_unix_ms()).await,
                )),
                Err(error) => SocketReply::Send(binary_frame(&protocol_error(
                    ProtocolErrorCode::InvalidMessage,
                    Vec::new(),
                    error.to_string(),
                    now_unix_ms(),
                ))),
            },
            other => reply_to(&other, now_unix_ms()),
        };
        match reply {
            SocketReply::Send(reply) => {
                if outbound.try_send(reply).is_err() {
                    warn!("peer is not draining its outbound queue");
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
