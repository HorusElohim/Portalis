//! In-memory ports for tests and local development.
//!
//! These are deliberately simple: a map behind a mutex, a clock that only
//! moves when a test moves it, and randomness a test can dictate. They make
//! the identity rules provable without `MongoDB`.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::ports::{
    Clock, DeviceId, DeviceRecord, DeviceRepository, RandomSource, RepositoryError, UserId,
    UserRecord, UserRepository,
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

    fn lock(&self) -> std::sync::MutexGuard<'_, u64> {
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
    bytes: Mutex<std::collections::VecDeque<u8>>,
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

/// Users held in memory, enforcing the unique-handle index.
#[derive(Debug, Default)]
pub struct InMemoryUsers {
    users: Mutex<Vec<UserRecord>>,
}

impl InMemoryUsers {
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<UserRecord>> {
        self.users.lock().expect("the user store is not poisoned")
    }
}

impl UserRepository for InMemoryUsers {
    fn insert_user(
        &self,
        user: UserRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let result = {
            let mut users = self.lock();
            let taken = users.iter().any(|existing| {
                existing.normalized_username == user.normalized_username
                    && existing.discriminator == user.discriminator
            });
            if taken {
                Err(RepositoryError::HandleTaken)
            } else {
                users.push(user);
                Ok(())
            }
        };
        async move { result }
    }

    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        let found = self
            .lock()
            .iter()
            .find(|user| user.user_id == user_id)
            .cloned();
        async move { Ok(found) }
    }
}

/// Devices held in memory, enforcing the unique-device index.
#[derive(Debug, Default)]
pub struct InMemoryDevices {
    devices: Mutex<HashMap<DeviceId, DeviceRecord>>,
}

impl InMemoryDevices {
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<DeviceId, DeviceRecord>> {
        self.devices
            .lock()
            .expect("the device store is not poisoned")
    }
}

impl DeviceRepository for InMemoryDevices {
    fn insert_device(
        &self,
        device: DeviceRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let result = {
            let mut devices = self.lock();
            match devices.entry(device.device_id) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    Err(RepositoryError::DeviceExists)
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(device);
                    Ok(())
                }
            }
        };
        async move { result }
    }

    fn find_device(
        &self,
        device_id: DeviceId,
    ) -> impl std::future::Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send
    {
        let found = self.lock().get(&device_id).cloned();
        async move { Ok(found) }
    }

    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        if let Some(device) = self.lock().get_mut(&device_id) {
            device.last_authenticated_at_unix_ms = Some(at_unix_ms);
        }
        async move { Ok(()) }
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        if let Some(device) = self.lock().get_mut(&device_id) {
            device.revoked_at_unix_ms = Some(at_unix_ms);
        }
        async move { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn the_user_store_enforces_unique_handles() {
        let users = InMemoryUsers::default();
        let user = UserRecord {
            user_id: [1; 16],
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ms: 1,
        };
        assert!(users.is_empty());

        assert_eq!(users.insert_user(user.clone()).await, Ok(()));
        assert_eq!(
            users
                .insert_user(UserRecord {
                    user_id: [2; 16],
                    ..user.clone()
                })
                .await,
            Err(RepositoryError::HandleTaken)
        );

        assert_eq!(users.len(), 1);
        assert_eq!(users.find_user([1; 16]).await, Ok(Some(user)));
        assert_eq!(users.find_user([9; 16]).await, Ok(None));
    }

    #[tokio::test]
    async fn the_device_store_enforces_unique_devices_and_records_changes() {
        let devices = InMemoryDevices::default();
        let device = DeviceRecord {
            device_id: [1; 32],
            user_id: [2; 16],
            public_key: [3; 32],
            created_at_unix_ms: 1,
            last_authenticated_at_unix_ms: None,
            revoked_at_unix_ms: None,
        };
        assert!(devices.is_empty());

        assert_eq!(devices.insert_device(device.clone()).await, Ok(()));
        assert_eq!(
            devices.insert_device(device.clone()).await,
            Err(RepositoryError::DeviceExists)
        );
        assert_eq!(devices.len(), 1);

        assert_eq!(devices.touch_device([1; 32], 42).await, Ok(()));
        assert_eq!(devices.revoke_device([1; 32], 43).await, Ok(()));
        let stored = devices
            .find_device([1; 32])
            .await
            .expect("stored")
            .expect("present");
        assert_eq!(stored.last_authenticated_at_unix_ms, Some(42));
        assert_eq!(stored.revoked_at_unix_ms, Some(43));
        assert!(stored.is_revoked());

        // Updating a device that is not there is a no-op, not an error.
        assert_eq!(devices.touch_device([9; 32], 1).await, Ok(()));
        assert_eq!(devices.revoke_device([9; 32], 1).await, Ok(()));
        assert_eq!(devices.find_device([9; 32]).await, Ok(None));
    }
}
