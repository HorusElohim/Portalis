//! The outside world, as seen from the domain.
//!
//! Time, randomness, and storage are all injected. That keeps the rules in
//! this crate deterministic under test: no sleeping, no network, no database.
//! Futures are spelled out as `impl Future + Send` rather than `async fn` so
//! implementations stay usable from a spawned task.

use std::future::Future;

use portalis_nexus_protocol::{DEVICE_ID_BYTES, DEVICE_KEY_BYTES, USER_ID_BYTES};
use thiserror::Error;

use crate::friendship::{FriendshipEdge, FriendshipRecord};

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
    /// A write lost a race: the stored version moved after it was read. The
    /// caller re-reads and re-applies rather than overwriting.
    #[error("the record changed since it was read")]
    VersionConflict,
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

/// Durable identity storage.
///
/// Users and devices share one trait because registration has to write both
/// or neither: a user whose first device is missing holds a handle it can
/// never authenticate with. Splitting them would leave no place to express
/// that, so [`IdentityRepository::insert_registration`] owns the pair.
/// Looking users up, which friends and presence need without touching devices.
///
/// Kept separate from [`IdentityRepository`] so a caller that only reads users
/// does not depend on device enrolment or revocation.
pub trait UserDirectory: Send + Sync {
    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send;

    /// Looks a user up by the indexed form of their handle.
    fn find_user_by_handle(
        &self,
        normalized_username: &str,
        discriminator: &str,
    ) -> impl Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send;
}

pub trait IdentityRepository: UserDirectory {
    /// Inserts a user and its first device as one atomic unit.
    ///
    /// Fails with [`RepositoryError::HandleTaken`] when the handle is already
    /// claimed, which allocation answers by trying another discriminator, or
    /// [`RepositoryError::DeviceExists`] when the device is already enrolled.
    fn insert_registration(
        &self,
        user: UserRecord,
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

/// Durable friendship storage, one row per canonical edge.
pub trait FriendRepository: Send + Sync {
    fn find_friendship(
        &self,
        edge: FriendshipEdge,
    ) -> impl Future<Output = Result<Option<FriendshipRecord>, RepositoryError>> + Send;

    /// Writes a friendship only while the stored version still matches
    /// `expected_version`, which is how concurrent commands stay deterministic.
    /// A version of zero means the edge must not exist yet.
    ///
    /// Fails with [`RepositoryError::VersionConflict`] when it has moved.
    fn save_friendship(
        &self,
        record: FriendshipRecord,
        expected_version: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Every friendship joining `user`, in no particular order.
    fn list_friendships(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<FriendshipRecord>, RepositoryError>> + Send;
}

// Shared-ownership delegations, so one store can back several services. The
// server holds a single identity store that both identity and friend rules
// read, which is why these exist.
impl<T: UserDirectory> UserDirectory for std::sync::Arc<T> {
    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        T::find_user(self, user_id)
    }

    fn find_user_by_handle(
        &self,
        normalized_username: &str,
        discriminator: &str,
    ) -> impl Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        T::find_user_by_handle(self, normalized_username, discriminator)
    }
}

impl<T: IdentityRepository> IdentityRepository for std::sync::Arc<T> {
    fn insert_registration(
        &self,
        user: UserRecord,
        device: DeviceRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::insert_registration(self, user, device)
    }

    fn find_device(
        &self,
        device_id: DeviceId,
    ) -> impl Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send {
        T::find_device(self, device_id)
    }

    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::touch_device(self, device_id, at_unix_ms)
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::revoke_device(self, device_id, at_unix_ms)
    }
}

impl<T: FriendRepository> FriendRepository for std::sync::Arc<T> {
    fn find_friendship(
        &self,
        edge: FriendshipEdge,
    ) -> impl Future<Output = Result<Option<FriendshipRecord>, RepositoryError>> + Send {
        T::find_friendship(self, edge)
    }

    fn save_friendship(
        &self,
        record: FriendshipRecord,
        expected_version: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::save_friendship(self, record, expected_version)
    }

    fn list_friendships(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<FriendshipRecord>, RepositoryError>> + Send {
        T::list_friendships(self, user)
    }
}
