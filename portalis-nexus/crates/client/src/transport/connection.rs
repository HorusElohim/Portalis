//! The reader, writer, and supervisor tasks behind one client handle.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use portalis_nexus_protocol::v1::{Envelope, ServerHello};
use portalis_nexus_protocol::{MAX_OUTBOUND_QUEUE, decode_frame, format_id};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::pending::PendingRequests;
use crate::protocol::ClientProtocol;
use crate::reconnect::ReconnectPolicy;
use crate::transport::Socket;
use crate::transport::handshake::handshake_with_retry;

/// State shared between the caller's handle and its supervisor task.
pub(crate) struct Shared {
    pub(crate) pending: PendingRequests,
    pub(crate) events: mpsc::Sender<Envelope>,
    pub(crate) protocol: ClientProtocol,
    pub(crate) request_timeout: Duration,
    pub(crate) shutdown: watch::Sender<bool>,
    live: Mutex<Option<Live>>,
}

/// The currently connected socket's send side and negotiated hello.
struct Live {
    outbound: mpsc::Sender<Message>,
    hello: ServerHello,
}

type Message = tokio_tungstenite::tungstenite::Message;

impl Shared {
    pub(crate) fn new(events: mpsc::Sender<Envelope>, request_timeout: Duration) -> Self {
        Self {
            pending: PendingRequests::default(),
            events,
            protocol: ClientProtocol::default(),
            request_timeout,
            shutdown: watch::Sender::new(false),
            live: Mutex::new(None),
        }
    }

    pub(crate) fn outbound(&self) -> Option<mpsc::Sender<Message>> {
        self.live().as_ref().map(|live| live.outbound.clone())
    }

    pub(crate) fn hello(&self) -> Option<ServerHello> {
        self.live().as_ref().map(|live| live.hello.clone())
    }

    fn set_live(&self, live: Live) {
        *self.live() = Some(live);
    }

    fn clear_live(&self) {
        self.live().take();
    }

    fn live(&self) -> MutexGuard<'_, Option<Live>> {
        // Never held across an await, so poisoning would mean a bug elsewhere.
        self.live
            .lock()
            .expect("the live connection slot is never poisoned")
    }
}

/// The reader, writer, and queue belonging to one live socket.
pub(crate) struct Tasks {
    reader: JoinHandle<()>,
    writer: JoinHandle<()>,
    outbound: mpsc::Sender<Message>,
}

/// Publishes a connection and starts its reader and writer tasks.
pub(crate) fn start_connection(
    shared: &Arc<Shared>,
    (socket, hello): (Socket, ServerHello),
) -> Tasks {
    let (sink, stream) = socket.split();
    let (outbound, inbox) = mpsc::channel(MAX_OUTBOUND_QUEUE);
    debug!(
        connection_id = %format_id(&hello.connection_id),
        "Nexus connection established"
    );
    shared.set_live(Live {
        outbound: outbound.clone(),
        hello,
    });

    Tasks {
        writer: tokio::spawn(write_socket(sink, inbox)),
        reader: tokio::spawn(read_socket(Arc::clone(shared), stream)),
        outbound,
    }
}

/// Keeps one connection live until the handle shuts down or retries run out.
pub(crate) async fn supervise(
    shared: Arc<Shared>,
    endpoint: String,
    policy: ReconnectPolicy,
    first: Tasks,
) {
    let mut shutdown = shared.shutdown.subscribe();
    let mut next = Some(first);
    loop {
        let tasks = if let Some(tasks) = next.take() {
            tasks
        } else {
            let attempt = tokio::select! {
                _ = shutdown.changed() => return,
                attempt = handshake_with_retry(&endpoint, &policy) => attempt,
            };
            match attempt {
                Ok(connection) => start_connection(&shared, connection),
                Err(error) => {
                    warn!(%error, "Nexus client stopped reconnecting");
                    return;
                }
            }
        };
        run_connection(&shared, tasks, &mut shutdown).await;
        if *shutdown.borrow() {
            return;
        }
        debug!("Nexus connection ended; reconnecting");
    }
}

/// Runs one connection until its reader ends or the handle shuts down.
async fn run_connection(shared: &Arc<Shared>, tasks: Tasks, shutdown: &mut watch::Receiver<bool>) {
    let Tasks {
        mut reader,
        writer,
        outbound,
    } = tasks;

    tokio::select! {
        _ = &mut reader => {}
        _ = shutdown.changed() => reader.abort(),
    }

    // Dropping every queue sender lets the writer send its close frame.
    drop(outbound);
    shared.clear_live();
    let _ = writer.await;
    shared.pending.cancel_all();
}

/// Routes inbound envelopes to their waiting request, or to the event stream.
async fn read_socket(shared: Arc<Shared>, mut stream: SplitStream<Socket>) {
    while let Some(message) = stream.next().await {
        let Ok(message) = message else {
            return;
        };
        match message {
            Message::Binary(frame) => {
                let Ok(envelope) = decode_frame(&frame) else {
                    warn!("Nexus server sent an invalid frame");
                    return;
                };
                if let Some(event) = shared.pending.route(envelope) {
                    if shared.events.try_send(event).is_err() {
                        warn!("Nexus event stream is full or closed");
                        return;
                    }
                }
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            Message::Text(_) | Message::Close(_) => return,
        }
    }
}

/// Writes queued messages, then closes the socket once the queue is dropped.
async fn write_socket(mut sink: SplitSink<Socket, Message>, mut inbox: mpsc::Receiver<Message>) {
    while let Some(message) = inbox.recv().await {
        if sink.send(message).await.is_err() {
            return;
        }
    }
    let _ = sink.send(Message::Close(None)).await;
}
