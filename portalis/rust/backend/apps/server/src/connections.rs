//! Where to reach a live connection.
//!
//! Presence and future events have to push to a specific socket rather than
//! answer a request, so each connection publishes its outbound queue here for
//! as long as it lasts. The queue is the same bounded one the writer drains,
//! so a peer that stops reading cannot make the server hold events for it.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use axum::extract::ws::Message;
use portalis_nexus_server_core::ConnectionId;
use tokio::sync::mpsc;

/// The outbound queue of every live connection.
#[derive(Debug, Default)]
pub struct Connections {
    outbound: Mutex<HashMap<ConnectionId, mpsc::Sender<Message>>>,
}

impl Connections {
    /// Publishes where to reach `connection`.
    pub fn register(&self, connection: ConnectionId, outbound: mpsc::Sender<Message>) {
        self.lock().insert(connection, outbound);
    }

    /// Forgets a connection that has ended.
    pub fn forget(&self, connection: ConnectionId) {
        self.lock().remove(&connection);
    }

    /// Pushes one message to a connection.
    ///
    /// Returns whether it was queued. A full queue means the peer is not
    /// reading, and an event is dropped rather than allowed to block the
    /// server or grow without bound; the peer refreshes state on reconnect.
    pub fn send(&self, connection: ConnectionId, message: Message) -> bool {
        let Some(outbound) = self.lock().get(&connection).cloned() else {
            return false;
        };
        outbound.try_send(message).is_ok()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<ConnectionId, mpsc::Sender<Message>>> {
        // Never held across an await, so poisoning would mean a bug elsewhere.
        self.outbound
            .lock()
            .expect("the connection registry is not poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: ConnectionId = [10; 16];
    const LAPTOP: ConnectionId = [11; 16];

    fn note() -> Message {
        Message::Binary(vec![1, 2, 3].into())
    }

    #[tokio::test]
    async fn a_registered_connection_receives_what_is_sent_to_it() {
        let connections = Connections::default();
        let (outbound, mut inbox) = mpsc::channel(4);
        assert!(connections.is_empty());

        connections.register(PHONE, outbound);

        assert_eq!(connections.len(), 1);
        assert!(connections.send(PHONE, note()));
        assert_eq!(inbox.recv().await, Some(note()));
    }

    #[tokio::test]
    async fn sending_to_an_unknown_connection_reports_it() {
        let connections = Connections::default();

        assert!(!connections.send(LAPTOP, note()));
    }

    #[tokio::test]
    async fn a_forgotten_connection_is_no_longer_reachable() {
        let connections = Connections::default();
        let (outbound, _inbox) = mpsc::channel(4);
        connections.register(PHONE, outbound);

        connections.forget(PHONE);

        assert!(connections.is_empty());
        assert!(!connections.send(PHONE, note()));
    }

    #[tokio::test]
    async fn a_peer_that_stopped_reading_loses_the_event() {
        let connections = Connections::default();
        let (outbound, inbox) = mpsc::channel(1);
        connections.register(PHONE, outbound);

        assert!(connections.send(PHONE, note()), "the queue has room");
        assert!(
            !connections.send(PHONE, note()),
            "a full queue drops the event rather than blocking"
        );

        drop(inbox);
        assert!(!connections.send(PHONE, note()), "a closed queue too");
    }
}
