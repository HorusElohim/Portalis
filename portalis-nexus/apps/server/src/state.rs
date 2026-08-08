//! Shared state carried by every request handler.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
use portalis_nexus_server_core::ProtocolPolicy;

use crate::shutdown::Shutdown;

#[derive(Clone, Debug)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    protocol_policy: ProtocolPolicy,
    shutdown: Shutdown,
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
            shutdown: Shutdown::default(),
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

    #[must_use]
    pub fn shutdown(&self) -> &Shutdown {
        &self.shutdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_is_opt_in() {
        let state = AppState::default();
        assert!(!state.is_ready());

        state.mark_ready();

        assert!(state.is_ready());
        assert!(!state.shutdown().is_draining());
    }
}
