//! Shared state carried by every request handler.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
use portalis_nexus_server_core::ProtocolPolicy;

use crate::config::DEFAULT_SERVER_AUTHORITY;
use crate::identity::{DefaultStore, NexusIdentities, identities};
use crate::shutdown::Shutdown;

#[derive(Clone)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    protocol_policy: ProtocolPolicy,
    shutdown: Shutdown,
    identities: Arc<NexusIdentities<DefaultStore>>,
    /// The host clients believe they are talking to. Signatures are bound to
    /// it, so a signature captured by one deployment cannot be replayed
    /// against another.
    server_authority: Arc<str>,
}

/// Reports configuration only; the identity service holds no printable state.
impl std::fmt::Debug for AppState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppState")
            .field("ready", &self.is_ready())
            .field("server_authority", &self.server_authority)
            .finish_non_exhaustive()
    }
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
            identities: Arc::new(identities(DefaultStore::default())),
            server_authority: Arc::from(DEFAULT_SERVER_AUTHORITY),
        }
    }
}

impl AppState {
    /// Binds this server to the authority clients sign against.
    #[must_use]
    pub fn with_server_authority(mut self, authority: &str) -> Self {
        self.server_authority = Arc::from(authority);
        self
    }

    #[must_use]
    pub fn server_authority(&self) -> &str {
        &self.server_authority
    }

    #[must_use]
    pub fn identities(&self) -> &NexusIdentities<DefaultStore> {
        &self.identities
    }

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

    #[test]
    fn carries_the_authority_signatures_are_bound_to() {
        let state = AppState::default();
        assert_eq!(state.server_authority(), DEFAULT_SERVER_AUTHORITY);

        let bound = state.with_server_authority("nexus.example");

        assert_eq!(bound.server_authority(), "nexus.example");
        assert!(format!("{bound:?}").contains("nexus.example"));
    }
}
