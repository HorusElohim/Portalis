//! The concrete services this server runs.

use portalis_nexus_server_core::{
    FriendRepository, FriendService, IdentityRepository, IdentityService, InMemoryIdentities,
    UserDirectory,
};

use crate::environment::{OsRandom, SystemClock};

/// The identity rules bound to this server's clock and random source.
///
/// The store stays generic so the durable adapter can replace the in-memory
/// one without touching the socket, the session, or the protocol.
pub type NexusIdentities<S> = IdentityService<S, SystemClock, OsRandom>;

/// The store used until the durable adapter lands.
///
/// Shared, because identity and friend rules must read the same users: two
/// stores would mean a registered user no one could befriend.
pub type DefaultStore = std::sync::Arc<InMemoryIdentities>;

#[must_use]
pub fn identities<S: IdentityRepository>(store: S) -> NexusIdentities<S> {
    IdentityService::new(store, SystemClock, OsRandom)
}

/// The friend rules bound to this server's clock.
pub type NexusFriends<S> = FriendService<S, SystemClock>;

#[must_use]
pub fn friends<S: FriendRepository + UserDirectory>(store: S) -> NexusFriends<S> {
    FriendService::new(store, SystemClock)
}
