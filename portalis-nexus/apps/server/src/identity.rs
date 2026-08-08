//! The concrete identity service this server runs.

use portalis_nexus_server_core::{IdentityRepository, IdentityService, InMemoryIdentities};

use crate::environment::{OsRandom, SystemClock};

/// The identity rules bound to this server's clock and random source.
///
/// The store stays generic so the durable adapter can replace the in-memory
/// one without touching the socket, the session, or the protocol.
pub type NexusIdentities<S> = IdentityService<S, SystemClock, OsRandom>;

/// The store used until the durable adapter lands.
pub type DefaultStore = InMemoryIdentities;

#[must_use]
pub fn identities<S: IdentityRepository>(store: S) -> NexusIdentities<S> {
    IdentityService::new(store, SystemClock, OsRandom)
}
