//! Shared state carried by every request handler.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;
use portalis_nexus_server_core::{PresenceRegistry, ProtocolPolicy};

use crate::config::DEFAULT_SERVER_AUTHORITY;
use crate::connections::Connections;
use crate::identity::{
    DefaultStore, NexusEnvelopes, NexusFriends, NexusIdentities, envelopes, friends, identities,
};
use crate::shutdown::Shutdown;
use crate::store::NexusStore;

#[derive(Clone)]
pub struct AppState {
    ready: Arc<AtomicBool>,
    protocol_policy: ProtocolPolicy,
    shutdown: Shutdown,
    store: DefaultStore,
    identities: Arc<NexusIdentities<DefaultStore>>,
    friends: Arc<NexusFriends<DefaultStore>>,
    envelopes: Arc<NexusEnvelopes<DefaultStore>>,
    presence: Arc<PresenceRegistry>,
    connections: Arc<Connections>,
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
        // One store behind both services, so a user registered through
        // identity is immediately findable as a friend.
        let store = DefaultStore::default();
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            protocol_policy: ProtocolPolicy::new(
                CURRENT_PROTOCOL_VERSION,
                CURRENT_PROTOCOL_VERSION,
            )
            .expect("the current protocol version is a valid range"),
            shutdown: Shutdown::default(),
            identities: Arc::new(identities(Arc::clone(&store))),
            friends: Arc::new(friends(Arc::clone(&store))),
            envelopes: Arc::new(envelopes(Arc::clone(&store))),
            store,
            presence: Arc::new(PresenceRegistry::default()),
            connections: Arc::new(Connections::default()),
            server_authority: Arc::from(DEFAULT_SERVER_AUTHORITY),
        }
    }
}

impl AppState {
    /// Builds a server over `store`, durable or otherwise.
    #[must_use]
    pub fn with_store(store: NexusStore) -> Self {
        let store = Arc::new(store);
        Self {
            identities: Arc::new(identities(Arc::clone(&store))),
            friends: Arc::new(friends(Arc::clone(&store))),
            envelopes: Arc::new(envelopes(Arc::clone(&store))),
            store,
            ..Self::default()
        }
    }

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

    #[must_use]
    pub fn friends(&self) -> &NexusFriends<DefaultStore> {
        &self.friends
    }

    #[must_use]
    pub fn envelopes(&self) -> &NexusEnvelopes<DefaultStore> {
        &self.envelopes
    }

    /// The store behind both services, so a caller can seed or fault it.
    #[must_use]
    pub fn store(&self) -> &DefaultStore {
        &self.store
    }

    #[must_use]
    pub fn presence(&self) -> &PresenceRegistry {
        &self.presence
    }

    #[must_use]
    pub fn connections(&self) -> &Connections {
        &self.connections
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

    #[tokio::test]
    async fn one_store_backs_both_identity_and_friend_rules() {
        let state = AppState::default();

        // Nobody is online and nothing is stored until a client arrives.
        assert_eq!(state.presence().online_users(), 0);
        assert!(!state.presence().is_online([1; 16]));
        assert_eq!(
            state.friends().list([1; 16]).await.expect("listed"),
            Vec::new()
        );
    }

    #[test]
    fn with_store_wires_identities_and_friends_to_the_given_backend() {
        let state = AppState::with_store(NexusStore::default());

        assert_eq!(state.store().kind(), "memory");
        // The default's readiness and presence still apply; only the store
        // and the services built over it are replaced.
        assert!(!state.is_ready());
        assert_eq!(state.presence().online_users(), 0);
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
