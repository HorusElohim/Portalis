//! The outside world, as seen from the domain.
//!
//! Time, randomness, and storage are all injected. That keeps the rules in
//! this crate deterministic under test: no sleeping, no network, no database.
//! Futures are spelled out as `impl Future + Send` rather than `async fn` so
//! implementations stay usable from a spawned task.

use std::future::Future;

use portalis_nexus_protocol::{DEVICE_ID_BYTES, DEVICE_KEY_BYTES, USER_ID_BYTES};
use thiserror::Error;

pub type UserId = [u8; USER_ID_BYTES];
pub type DeviceId = [u8; DEVICE_ID_BYTES];
pub type DeviceKey = [u8; DEVICE_KEY_BYTES];

/// Reads wall-clock time, which the domain never does for itself.
pub trait Clock: Send + Sync {
    fn now_unix_ms(&self) -> u64;
}

/// Fills a buffer with cryptographically random bytes.
pub trait RandomSource: Send + Sync {
    fn fill(&self, buffer: &mut [u8]);
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    /// The unique `(normalized_username, discriminator)` index rejected the
    /// write. Allocation treats this as "try another discriminator", never as
    /// a reason to scan for a free one.
    #[error("that handle is already taken")]
    HandleTaken,
    #[error("that device is already registered")]
    DeviceExists,
    #[error("the identity store is unavailable: {0}")]
    Unavailable(String),
}

/// A durable user record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserRecord {
    pub user_id: UserId,
    /// Display casing, as its owner typed it.
    pub username: String,
    /// The indexed form that makes a handle unique.
    pub normalized_username: String,
    pub discriminator: String,
    pub created_at_unix_ms: u64,
}

/// A durable record of one device authorized to act for a user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_id: DeviceId,
    pub user_id: UserId,
    pub public_key: DeviceKey,
    pub created_at_unix_ms: u64,
    pub last_authenticated_at_unix_ms: Option<u64>,
    pub revoked_at_unix_ms: Option<u64>,
}

impl DeviceRecord {
    #[must_use]
    pub fn is_revoked(&self) -> bool {
        self.revoked_at_unix_ms.is_some()
    }
}

pub trait UserRepository: Send + Sync {
    /// Inserts a user, failing with [`RepositoryError::HandleTaken`] when the
    /// handle is already claimed.
    fn insert_user(
        &self,
        user: UserRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send;
}

pub trait DeviceRepository: Send + Sync {
    /// Inserts a device, failing with [`RepositoryError::DeviceExists`] when
    /// the identifier is already enrolled.
    fn insert_device(
        &self,
        device: DeviceRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn find_device(
        &self,
        device_id: DeviceId,
    ) -> impl Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send;

    /// Records a successful authentication. Missing devices are ignored: the
    /// caller has already verified the device exists.
    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Revokes a device so it can no longer authenticate.
    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
