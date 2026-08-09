//! Who is online, aggregated across all of a user's devices.
//!
//! Presence is derived from live connections rather than stored: it is true
//! only while a socket is open, so persisting it would mean writing a lie on
//! every crash. A user is online while at least one of their devices is
//! connected, and the registry reports only the moments that change that, so
//! callers fan out one event per real transition rather than one per device.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

use portalis_nexus_protocol::CONNECTION_ID_BYTES;

use crate::ports::UserId;

/// Identifies one live socket, as issued in its `ServerHello`.
pub type ConnectionId = [u8; CONNECTION_ID_BYTES];

/// A moment worth telling a user's friends about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresenceChange {
    /// Their first device connected.
    CameOnline,
    /// Their last device went away.
    WentOffline,
}

/// Live connections, grouped by the user they authenticated as.
#[derive(Debug, Default)]
pub struct PresenceRegistry {
    state: Mutex<Presence>,
}

#[derive(Debug, Default)]
struct Presence {
    /// Only users with at least one live connection appear here, so an entry
    /// existing is the same as being online.
    connections: HashMap<UserId, HashSet<ConnectionId>>,
    last_seen: HashMap<UserId, u64>,
}

impl PresenceRegistry {
    /// Records that `connection` now speaks for `user`.
    ///
    /// Returns [`PresenceChange::CameOnline`] only for their first device, so
    /// a second phone does not announce the same person twice. Re-registering
    /// a connection already counted changes nothing.
    #[must_use]
    pub fn arrive(&self, user: UserId, connection: ConnectionId) -> Option<PresenceChange> {
        let mut state = self.lock();
        let devices = state.connections.entry(user).or_default();
        let first = devices.is_empty();
        let added = devices.insert(connection);
        if first && added {
            state.last_seen.remove(&user);
            return Some(PresenceChange::CameOnline);
        }
        None
    }

    /// Records that `connection` has gone.
    ///
    /// Returns [`PresenceChange::WentOffline`] only when it was the user's
    /// last one. Departing twice, or departing a connection that was never
    /// counted, changes nothing.
    #[must_use]
    pub fn depart(
        &self,
        user: UserId,
        connection: ConnectionId,
        at_unix_ms: u64,
    ) -> Option<PresenceChange> {
        let mut state = self.lock();
        let devices = state.connections.get_mut(&user)?;
        if !devices.remove(&connection) || !devices.is_empty() {
            return None;
        }
        state.connections.remove(&user);
        state.last_seen.insert(user, at_unix_ms);
        Some(PresenceChange::WentOffline)
    }

    /// Every live connection speaking for `user`, for fanning an event out to
    /// each of their devices.
    #[must_use]
    pub fn connections_of(&self, user: UserId) -> Vec<ConnectionId> {
        self.lock()
            .connections
            .get(&user)
            .map(|devices| devices.iter().copied().collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_online(&self, user: UserId) -> bool {
        self.lock().connections.contains_key(&user)
    }

    /// How many devices are speaking for `user` right now.
    #[must_use]
    pub fn device_count(&self, user: UserId) -> usize {
        self.lock().connections.get(&user).map_or(0, HashSet::len)
    }

    /// When `user` was last online, or `None` while they still are.
    #[must_use]
    pub fn last_seen(&self, user: UserId) -> Option<u64> {
        let state = self.lock();
        if state.connections.contains_key(&user) {
            return None;
        }
        state.last_seen.get(&user).copied()
    }

    /// How many users are online, for metrics.
    #[must_use]
    pub fn online_users(&self) -> usize {
        self.lock().connections.len()
    }

    fn lock(&self) -> MutexGuard<'_, Presence> {
        // Never held across an await, so poisoning would mean a bug elsewhere.
        self.state
            .lock()
            .expect("the presence registry is not poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADA: UserId = [1; 16];
    const GRACE: UserId = [2; 16];
    const PHONE: ConnectionId = [10; 16];
    const LAPTOP: ConnectionId = [11; 16];
    const NOW: u64 = 1_700_000_000_000;

    #[test]
    fn the_first_device_brings_a_user_online() {
        let registry = PresenceRegistry::default();
        assert!(!registry.is_online(ADA));
        assert_eq!(registry.online_users(), 0);

        assert_eq!(
            registry.arrive(ADA, PHONE),
            Some(PresenceChange::CameOnline)
        );

        assert!(registry.is_online(ADA));
        assert_eq!(registry.device_count(ADA), 1);
        assert_eq!(registry.connections_of(ADA), vec![PHONE]);
        assert!(registry.connections_of(GRACE).is_empty());
        assert_eq!(registry.online_users(), 1);
        assert_eq!(registry.last_seen(ADA), None);
    }

    #[test]
    fn more_devices_do_not_announce_the_same_person_again() {
        let registry = PresenceRegistry::default();
        registry.arrive(ADA, PHONE).expect("came online");

        assert_eq!(registry.arrive(ADA, LAPTOP), None);
        // Re-registering a connection already counted is a no-op.
        assert_eq!(registry.arrive(ADA, PHONE), None);

        assert_eq!(registry.device_count(ADA), 2);
        assert_eq!(registry.online_users(), 1);
        let mut reached = registry.connections_of(ADA);
        reached.sort_unstable();
        assert_eq!(
            reached,
            vec![PHONE, LAPTOP],
            "an event reaches every device"
        );
    }

    #[test]
    fn only_the_last_device_leaving_takes_a_user_offline() {
        let registry = PresenceRegistry::default();
        registry.arrive(ADA, PHONE).expect("came online");
        assert_eq!(registry.arrive(ADA, LAPTOP), None);

        assert_eq!(registry.depart(ADA, PHONE, NOW), None);
        assert!(registry.is_online(ADA), "a laptop is still connected");

        assert_eq!(
            registry.depart(ADA, LAPTOP, NOW + 1),
            Some(PresenceChange::WentOffline)
        );
        assert!(!registry.is_online(ADA));
        assert_eq!(registry.device_count(ADA), 0);
        assert_eq!(registry.online_users(), 0);
        assert_eq!(registry.last_seen(ADA), Some(NOW + 1));
    }

    #[test]
    fn departing_twice_or_unknown_changes_nothing() {
        let registry = PresenceRegistry::default();
        registry.arrive(ADA, PHONE).expect("came online");
        registry.depart(ADA, PHONE, NOW).expect("went offline");

        assert_eq!(registry.depart(ADA, PHONE, NOW + 1), None);
        assert_eq!(registry.depart(GRACE, PHONE, NOW + 1), None);
        // A connection that was never counted must not evict the others.
        registry.arrive(GRACE, PHONE).expect("Grace came online");
        assert_eq!(registry.depart(GRACE, LAPTOP, NOW + 2), None);
        assert!(registry.is_online(GRACE));
        // The earlier time stands: nothing above changed when Ada was seen.
        assert_eq!(registry.last_seen(ADA), Some(NOW));
    }

    #[test]
    fn coming_back_clears_the_last_seen_time() {
        let registry = PresenceRegistry::default();
        registry.arrive(ADA, PHONE).expect("came online");
        registry.depart(ADA, PHONE, NOW).expect("went offline");
        assert_eq!(registry.last_seen(ADA), Some(NOW));

        assert_eq!(
            registry.arrive(ADA, LAPTOP),
            Some(PresenceChange::CameOnline)
        );

        assert_eq!(
            registry.last_seen(ADA),
            None,
            "someone online has no last-seen time"
        );
    }

    #[test]
    fn users_are_tracked_apart() {
        let registry = PresenceRegistry::default();

        registry.arrive(ADA, PHONE).expect("Ada online");
        assert_eq!(
            registry.arrive(GRACE, PHONE),
            Some(PresenceChange::CameOnline),
            "the same connection id for a different user is its own device"
        );

        assert_eq!(registry.online_users(), 2);
        assert_eq!(
            registry.depart(ADA, PHONE, NOW),
            Some(PresenceChange::WentOffline)
        );
        assert!(registry.is_online(GRACE));
        assert_eq!(registry.last_seen(GRACE), None);
    }
}
