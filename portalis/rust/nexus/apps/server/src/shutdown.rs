//! Graceful draining of upgraded WebSocket connections.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

/// How long shutdown waits for live sockets before closing them anyway.
pub const GRACEFUL_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Broadcasts the draining signal and tracks how many sockets remain live.
///
/// Upgraded WebSocket connections outlive the HTTP connections that created
/// them, so `axum`'s graceful shutdown cannot wait for them. Every socket holds
/// one registration for its lifetime; [`Shutdown::drain`] asks them all to
/// close and resolves once the last registration is dropped.
#[derive(Clone, Debug)]
pub struct Shutdown {
    signal: Arc<watch::Sender<bool>>,
}

impl Default for Shutdown {
    fn default() -> Self {
        Self {
            signal: Arc::new(watch::Sender::new(false)),
        }
    }
}

impl Shutdown {
    /// Registers one live socket, which drains when the receiver reports a
    /// change and stays counted until the receiver is dropped.
    ///
    /// A socket that registers after draining began is reported as changed
    /// straight away, so it closes instead of waiting for a signal that has
    /// already been sent.
    #[must_use]
    pub fn register(&self) -> watch::Receiver<bool> {
        let mut socket = self.signal.subscribe();
        if *socket.borrow_and_update() {
            socket.mark_changed();
        }
        socket
    }

    #[must_use]
    pub fn is_draining(&self) -> bool {
        *self.signal.borrow()
    }

    /// Signals every registered socket to close and waits for them to finish.
    ///
    /// Callers bound this with [`GRACEFUL_DRAIN_TIMEOUT`] so an unresponsive
    /// peer cannot hold back process shutdown.
    pub async fn drain(&self) {
        // `send_replace` records the state even when no socket is listening.
        self.signal.send_replace(true);
        self.signal.closed().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn drain_completes_immediately_without_sockets() {
        let shutdown = Shutdown::default();

        shutdown.drain().await;

        assert!(shutdown.is_draining());
    }

    #[tokio::test]
    async fn sockets_registered_after_draining_close_immediately() {
        let shutdown = Shutdown::default();
        shutdown.drain().await;

        let mut late = shutdown.register();

        assert!(late.changed().await.is_ok());
    }

    #[tokio::test]
    async fn drain_signals_and_waits_for_live_sockets() {
        let shutdown = Shutdown::default();
        let mut socket = shutdown.register();
        assert!(!shutdown.is_draining());

        let drained = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.drain().await }
        });
        socket.changed().await.expect("draining signal arrives");

        assert!(shutdown.is_draining());
        assert!(!drained.is_finished());
        drop(socket);
        drained.await.expect("drain completes once sockets close");
    }
}
