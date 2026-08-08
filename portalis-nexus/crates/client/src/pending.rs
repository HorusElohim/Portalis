//! The bounded registry that correlates requests with their responses.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use portalis_nexus_protocol::MAX_PENDING_REQUESTS;
use portalis_nexus_protocol::v1::Envelope;
use tokio::sync::oneshot;

use crate::error::ClientError;

/// Correlates in-flight requests with the responses that answer them.
///
/// The registry is bounded, so a client cannot keep unbounded state for a
/// server that never replies. Every waiter is removed exactly once: by a
/// matching response, by [`PendingRequests::cancel`] when its caller times out,
/// or by [`PendingRequests::cancel_all`] when the connection ends.
#[derive(Debug)]
pub struct PendingRequests {
    waiters: Mutex<HashMap<Vec<u8>, oneshot::Sender<Envelope>>>,
    capacity: usize,
}

impl Default for PendingRequests {
    fn default() -> Self {
        Self::new(MAX_PENDING_REQUESTS)
    }
}

impl PendingRequests {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            waiters: Mutex::new(HashMap::new()),
            capacity,
        }
    }

    /// Registers one in-flight request and returns its response receiver.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::TooManyPendingRequests`] once the registry holds
    /// its configured maximum.
    pub fn register(
        &self,
        message_id: Vec<u8>,
    ) -> Result<oneshot::Receiver<Envelope>, ClientError> {
        let mut waiters = self.waiters();
        if waiters.len() >= self.capacity {
            return Err(ClientError::TooManyPendingRequests);
        }
        let (sender, receiver) = oneshot::channel();
        waiters.insert(message_id, sender);
        Ok(receiver)
    }

    /// Delivers one inbound envelope to the request that is waiting for it.
    ///
    /// Returns the envelope when nothing was waiting, which makes it a
    /// server-initiated event. A response whose caller already gave up is
    /// discarded rather than reported as an event.
    #[must_use]
    pub fn route(&self, envelope: Envelope) -> Option<Envelope> {
        if envelope.correlation_id.is_empty() {
            return Some(envelope);
        }
        let Some(waiter) = self.waiters().remove(&envelope.correlation_id) else {
            return Some(envelope);
        };
        let _ = waiter.send(envelope);
        None
    }

    /// Removes an abandoned request so a timeout cannot leak registry capacity.
    pub fn cancel(&self, message_id: &[u8]) {
        self.waiters().remove(message_id);
    }

    /// Fails every in-flight request, which a dropped connection cannot answer.
    pub fn cancel_all(&self) {
        self.waiters().clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.waiters().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn waiters(&self) -> MutexGuard<'_, HashMap<Vec<u8>, oneshot::Sender<Envelope>>> {
        // Never held across an await, so poisoning would mean a bug elsewhere.
        self.waiters
            .lock()
            .expect("the pending request registry is never poisoned")
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::new_message_id;
    use portalis_nexus_protocol::v1::envelope::Payload;
    use portalis_nexus_protocol::v1::{Ping, Pong};

    use super::*;

    fn response_to(message_id: &[u8], nonce: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: message_id.to_vec(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Pong(Pong { nonce })),
        }
    }

    fn event() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        }
    }

    #[tokio::test]
    async fn routes_a_response_to_its_waiting_request() {
        let pending = PendingRequests::default();
        let id = new_message_id();
        let waiter = pending.register(id.clone()).expect("registers");
        assert_eq!(pending.len(), 1);

        assert!(pending.route(response_to(&id, 7)).is_none());

        assert_eq!(
            waiter.await.expect("response delivered").payload,
            Some(Payload::Pong(Pong { nonce: 7 }))
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn returns_envelopes_nothing_is_waiting_for() {
        let pending = PendingRequests::default();
        let unsolicited = event();
        let orphan = response_to(&new_message_id(), 1);

        assert_eq!(pending.route(unsolicited.clone()), Some(unsolicited));
        assert_eq!(pending.route(orphan.clone()), Some(orphan));
        assert!(pending.is_empty());
    }

    #[test]
    fn discards_a_response_whose_caller_gave_up() {
        let pending = PendingRequests::default();
        let id = new_message_id();
        drop(pending.register(id.clone()).expect("registers"));

        assert!(pending.route(response_to(&id, 7)).is_none());
        assert!(pending.is_empty());
    }

    #[test]
    fn cancelling_frees_registry_capacity() {
        let pending = PendingRequests::new(1);
        let id = new_message_id();
        let _waiter = pending.register(id.clone()).expect("registers");

        assert_eq!(
            pending.register(new_message_id()).unwrap_err(),
            ClientError::TooManyPendingRequests
        );

        pending.cancel(&id);

        assert!(pending.is_empty());
        assert!(pending.register(new_message_id()).is_ok());
    }

    #[tokio::test]
    async fn cancel_all_fails_every_in_flight_request() {
        let pending = PendingRequests::default();
        let first = pending.register(new_message_id()).expect("registers");
        let second = pending.register(new_message_id()).expect("registers");

        pending.cancel_all();

        assert!(pending.is_empty());
        assert!(first.await.is_err());
        assert!(second.await.is_err());
    }
}
