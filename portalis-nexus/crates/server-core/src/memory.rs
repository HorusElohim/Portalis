//! In-memory ports for tests and local development.
//!
//! These are deliberately simple: one lock over both collections, a clock that
//! only moves when a test moves it, and randomness a test can dictate. They
//! make the identity rules provable without `MongoDB`.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use crate::ports::{
    Clock, DeviceId, DeviceRecord, IdentityRepository, RandomSource, RepositoryError, UserId,
    UserRecord,
};

/// A clock that stands still until a test advances it.
#[derive(Debug)]
pub struct FixedClock {
    now_unix_ms: Mutex<u64>,
}

impl FixedClock {
    #[must_use]
    pub fn new(now_unix_ms: u64) -> Self {
        Self {
            now_unix_ms: Mutex::new(now_unix_ms),
        }
    }

    pub fn advance(&self, millis: u64) {
        *self.lock() += millis;
    }

    pub fn set(&self, now_unix_ms: u64) {
        *self.lock() = now_unix_ms;
    }

    fn lock(&self) -> MutexGuard<'_, u64> {
        self.now_unix_ms
            .lock()
            .expect("the test clock is not poisoned")
    }
}

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> u64 {
        *self.lock()
    }
}

/// Randomness a test dictates, cycling through a scripted sequence.
#[derive(Debug)]
pub struct ScriptedRandom {
    bytes: Mutex<VecDeque<u8>>,
}

impl ScriptedRandom {
    /// Repeats `sequence` for as long as callers keep drawing from it.
    ///
    /// # Panics
    ///
    /// Panics when `sequence` is empty, which would leave nothing to draw.
    #[must_use]
    pub fn new(sequence: &[u8]) -> Self {
        assert!(!sequence.is_empty(), "scripted randomness needs bytes");
        Self {
            bytes: Mutex::new(sequence.iter().copied().collect()),
        }
    }
}

impl RandomSource for ScriptedRandom {
    fn fill(&self, buffer: &mut [u8]) {
        let mut bytes = self
            .bytes
            .lock()
            .expect("the scripted random source is not poisoned");
        for slot in buffer.iter_mut() {
            let byte = bytes.pop_front().unwrap_or_default();
            bytes.push_back(byte);
            *slot = byte;
        }
    }
}

/// Users and devices behind one lock, so a registration is genuinely atomic.
#[derive(Debug, Default)]
pub struct InMemoryIdentities {
    stored: Mutex<Stored>,
}

#[derive(Debug, Default)]
struct Stored {
    users: Vec<UserRecord>,
    devices: HashMap<DeviceId, DeviceRecord>,
}

impl Stored {
    fn insert_user(&mut self, user: UserRecord) -> Result<(), RepositoryError> {
        let taken = self.users.iter().any(|existing| {
            existing.normalized_username == user.normalized_username
                && existing.discriminator == user.discriminator
        });
        if taken {
            return Err(RepositoryError::HandleTaken);
        }
        self.users.push(user);
        Ok(())
    }

    fn insert_device(&mut self, device: DeviceRecord) -> Result<(), RepositoryError> {
        match self.devices.entry(device.device_id) {
            Entry::Occupied(_) => Err(RepositoryError::DeviceExists),
            Entry::Vacant(slot) => {
                slot.insert(device);
                Ok(())
            }
        }
    }
}

impl InMemoryIdentities {
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.lock().users.len()
    }

    #[must_use]
    pub fn device_count(&self) -> usize {
        self.lock().devices.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.user_count() == 0 && self.device_count() == 0
    }

    /// Enrols a device directly, for tests that need one without registering.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::DeviceExists`] when already enrolled.
    pub fn enrol_device(&self, device: DeviceRecord) -> Result<(), RepositoryError> {
        self.lock().insert_device(device)
    }

    /// Stores a user directly, for tests that need one without registering.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::HandleTaken`] when the handle is claimed.
    pub fn store_user(&self, user: UserRecord) -> Result<(), RepositoryError> {
        self.lock().insert_user(user)
    }

    fn lock(&self) -> MutexGuard<'_, Stored> {
        self.stored
            .lock()
            .expect("the identity store is not poisoned")
    }
}

impl IdentityRepository for InMemoryIdentities {
    fn insert_registration(
        &self,
        user: UserRecord,
        device: DeviceRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let result = {
            let mut stored = self.lock();
            // Both writes happen under one lock, and the handle is checked
            // first so a collision never enrols the device.
            stored
                .insert_user(user)
                .and_then(|()| match stored.insert_device(device) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        stored.users.pop();
                        Err(error)
                    }
                })
        };
        async move { result }
    }

    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        let found = self
            .lock()
            .users
            .iter()
            .find(|user| user.user_id == user_id)
            .cloned();
        async move { Ok(found) }
    }

    fn find_device(
        &self,
        device_id: DeviceId,
    ) -> impl std::future::Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send
    {
        let found = self.lock().devices.get(&device_id).cloned();
        async move { Ok(found) }
    }

    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        if let Some(device) = self.lock().devices.get_mut(&device_id) {
            device.last_authenticated_at_unix_ms = Some(at_unix_ms);
        }
        async move { Ok(()) }
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        if let Some(device) = self.lock().devices.get_mut(&device_id) {
            device.revoked_at_unix_ms = Some(at_unix_ms);
        }
        async move { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(discriminator: &str) -> UserRecord {
        UserRecord {
            user_id: [1; 16],
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: discriminator.to_owned(),
            created_at_unix_ms: 1,
        }
    }

    fn device(seed: u8) -> DeviceRecord {
        DeviceRecord {
            device_id: [seed; 32],
            user_id: [1; 16],
            public_key: [3; 32],
            created_at_unix_ms: 1,
            last_authenticated_at_unix_ms: None,
            revoked_at_unix_ms: None,
        }
    }

    #[test]
    fn the_clock_only_moves_when_told_to() {
        let clock = FixedClock::new(10);

        assert_eq!(clock.now_unix_ms(), 10);
        clock.advance(5);
        assert_eq!(clock.now_unix_ms(), 15);
        clock.set(1);
        assert_eq!(clock.now_unix_ms(), 1);
    }

    #[test]
    fn scripted_randomness_repeats_its_sequence() {
        let random = ScriptedRandom::new(&[1, 2]);
        let mut buffer = [0_u8; 5];

        random.fill(&mut buffer);

        assert_eq!(buffer, [1, 2, 1, 2, 1]);
    }

    #[tokio::test]
    async fn a_registration_stores_the_user_and_its_device() {
        let store = InMemoryIdentities::default();
        assert!(store.is_empty());

        assert_eq!(
            store.insert_registration(user("7Q2XZ"), device(1)).await,
            Ok(())
        );

        assert_eq!(store.user_count(), 1);
        assert_eq!(store.device_count(), 1);
        assert_eq!(store.find_user([1; 16]).await, Ok(Some(user("7Q2XZ"))));
        assert_eq!(store.find_device([1; 32]).await, Ok(Some(device(1))));
        assert_eq!(store.find_user([9; 16]).await, Ok(None));
        assert_eq!(store.find_device([9; 32]).await, Ok(None));
    }

    #[tokio::test]
    async fn a_taken_handle_leaves_nothing_behind() {
        let store = InMemoryIdentities::default();
        store
            .insert_registration(user("7Q2XZ"), device(1))
            .await
            .expect("first registration");

        assert_eq!(
            store.insert_registration(user("7Q2XZ"), device(2)).await,
            Err(RepositoryError::HandleTaken)
        );

        assert_eq!(store.user_count(), 1);
        assert_eq!(
            store.device_count(),
            1,
            "a rejected registration must not enrol its device"
        );
    }

    #[tokio::test]
    async fn an_enrolled_device_cannot_register_again() {
        let store = InMemoryIdentities::default();
        store
            .insert_registration(user("7Q2XZ"), device(1))
            .await
            .expect("first registration");

        assert_eq!(
            store.insert_registration(user("ABCDE"), device(1)).await,
            Err(RepositoryError::DeviceExists)
        );
        assert_eq!(
            store.user_count(),
            1,
            "a rejected registration must not claim its handle"
        );
    }

    #[tokio::test]
    async fn records_authentication_and_revocation() {
        let store = InMemoryIdentities::default();
        store.enrol_device(device(1)).expect("device enrolled");
        store.store_user(user("7Q2XZ")).expect("user stored");
        assert_eq!(
            store.enrol_device(device(1)),
            Err(RepositoryError::DeviceExists)
        );
        assert_eq!(
            store.store_user(user("7Q2XZ")),
            Err(RepositoryError::HandleTaken)
        );

        assert_eq!(store.touch_device([1; 32], 42).await, Ok(()));
        assert_eq!(store.revoke_device([1; 32], 43).await, Ok(()));

        let stored = store
            .find_device([1; 32])
            .await
            .expect("stored")
            .expect("present");
        assert_eq!(stored.last_authenticated_at_unix_ms, Some(42));
        assert_eq!(stored.revoked_at_unix_ms, Some(43));
        assert!(stored.is_revoked());

        // Updating a device that is not there is a no-op, not an error.
        assert_eq!(store.touch_device([9; 32], 1).await, Ok(()));
        assert_eq!(store.revoke_device([9; 32], 1).await, Ok(()));
    }
}
