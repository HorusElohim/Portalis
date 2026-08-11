//! The outside world, as seen from the domain.
//!
//! Time, randomness, and storage are all injected. That keeps the rules in
//! this crate deterministic under test: no sleeping, no network, no database.
//! Futures are spelled out as `impl Future + Send` rather than `async fn` so
//! implementations stay usable from a spawned task.

use std::future::Future;

use portalis_nexus_protocol::{
    DEVICE_ID_BYTES, DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, MAX_KEY_ENVELOPES_PER_PAGE,
    SHARE_ID_BYTES, USER_ID_BYTES,
};
use thiserror::Error;

use crate::friendship::{FriendshipEdge, FriendshipRecord};
use crate::share::{ShareRecord, SnapshotId};

pub type UserId = [u8; USER_ID_BYTES];
pub type DeviceId = [u8; DEVICE_ID_BYTES];
pub type DeviceKey = [u8; DEVICE_KEY_BYTES];
/// An X25519 public key, used only to receive encrypted share-key envelopes.
pub type EncryptionKey = [u8; ENCRYPTION_KEY_BYTES];
/// Client-generated and opaque to Nexus until M4 gives it a real record.
pub type ShareId = [u8; SHARE_ID_BYTES];

/// Reads wall-clock time, which the domain never does for itself.
pub trait Clock: Send + Sync {
    fn now_unix_ns(&self) -> u64;
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
    pub created_at_unix_ns: u64,
}

/// A durable record of one device authorized to act for a user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_id: DeviceId,
    pub user_id: UserId,
    pub public_key: DeviceKey,
    /// Where a share-key envelope for this device is encrypted to. Separate
    /// from `public_key` because signing and encryption use different curves.
    pub encryption_public_key: EncryptionKey,
    pub created_at_unix_ns: u64,
    pub last_authenticated_at_unix_ns: Option<u64>,
    pub revoked_at_unix_ns: Option<u64>,
}

impl DeviceRecord {
    #[must_use]
    pub fn is_revoked(&self) -> bool {
        self.revoked_at_unix_ns.is_some()
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

    /// Lists the non-revoked devices currently authorized for `user`.
    fn list_devices(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<DeviceRecord>, RepositoryError>> + Send;

    /// Enrols an additional device for a user who already exists, authorized
    /// by one of that user's other devices rather than by claiming a handle.
    ///
    /// Fails with [`RepositoryError::DeviceExists`] when the device is
    /// already enrolled.
    fn link_device(
        &self,
        device: DeviceRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Records a successful authentication. Missing devices are ignored: the
    /// caller has already verified the device exists.
    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Revokes a device so it can no longer authenticate.
    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
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

/// A share's random symmetric key, sealed to one device's X25519 public key.
///
/// Nexus stores and relays this opaquely: `ciphertext` is meaningless
/// without the recipient device's private key, which Nexus never holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEnvelopeRecord {
    pub share_id: ShareId,
    pub recipient_device_id: DeviceId,
    pub ephemeral_public_key: EncryptionKey,
    pub ciphertext: Vec<u8>,
    pub created_at_unix_ns: u64,
}

/// One deterministic page of envelopes for a recipient device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEnvelopePage {
    pub envelopes: Vec<KeyEnvelopeRecord>,
    /// The exclusive cursor for the next page, if another page exists.
    pub next_after_share_id: Option<ShareId>,
}

/// One immutable encrypted snapshot. The mutable share record only points at
/// the latest one; keeping revisions separately makes history append-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareSnapshotRecord {
    pub share_id: ShareId,
    pub revision: u64,
    pub snapshot_id: SnapshotId,
    pub capsule: Vec<u8>,
    pub capsule_signature: Vec<u8>,
    pub created_at_unix_ns: u64,
}

/// A private share edge. Ownership remains on [`ShareRecord`]; this only
/// records additional users who may discover and fetch it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareMembershipRecord {
    pub share_id: ShareId,
    pub user_id: UserId,
    pub granted_at_unix_ns: u64,
}

/// Durable encrypted-share storage.
pub trait ShareRepository: Send + Sync {
    fn find_share(
        &self,
        share_id: ShareId,
    ) -> impl Future<Output = Result<Option<ShareRecord>, RepositoryError>> + Send;

    /// Appends `snapshot` and moves the share head only if its revision still
    /// equals `expected_revision`. `None` means the share must not exist.
    fn save_publication(
        &self,
        share: ShareRecord,
        snapshot: ShareSnapshotRecord,
        expected_revision: Option<u64>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn find_snapshot(
        &self,
        share_id: ShareId,
        revision: u64,
    ) -> impl Future<Output = Result<Option<ShareSnapshotRecord>, RepositoryError>> + Send;

    fn grant_share_access(
        &self,
        membership: ShareMembershipRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Removes a membership edge, or reports success when there was none.
    ///
    /// Idempotent because a revocation that has already happened is the state
    /// the caller asked for, and an owner retrying one should not be told
    /// their member is somehow still there.
    fn revoke_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn has_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    fn list_authorized_shares(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<Vec<ShareRecord>, RepositoryError>> + Send;

    fn list_share_members(
        &self,
        share_id: ShareId,
    ) -> impl Future<Output = Result<Vec<UserId>, RepositoryError>> + Send;
}

/// Durable key-envelope storage, one row per share and recipient device.
pub trait EnvelopeRepository: Send + Sync {
    /// Stores `envelope`, replacing any earlier one for the same share and
    /// recipient device — a rotated share key or a retried push both land
    /// the same way rather than accumulating stale rows.
    fn put_key_envelope(
        &self,
        envelope: KeyEnvelopeRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// A bounded, bytewise-share-ID-ordered page addressed to one device.
    fn list_key_envelopes(
        &self,
        recipient_device_id: DeviceId,
        after_share_id: Option<ShareId>,
    ) -> impl Future<Output = Result<KeyEnvelopePage, RepositoryError>> + Send;
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

    fn list_devices(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<DeviceRecord>, RepositoryError>> + Send {
        T::list_devices(self, user)
    }

    fn link_device(
        &self,
        device: DeviceRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::link_device(self, device)
    }

    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::touch_device(self, device_id, at_unix_ns)
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::revoke_device(self, device_id, at_unix_ns)
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

impl<T: EnvelopeRepository> EnvelopeRepository for std::sync::Arc<T> {
    fn put_key_envelope(
        &self,
        envelope: KeyEnvelopeRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::put_key_envelope(self, envelope)
    }

    fn list_key_envelopes(
        &self,
        recipient_device_id: DeviceId,
        after_share_id: Option<ShareId>,
    ) -> impl Future<Output = Result<KeyEnvelopePage, RepositoryError>> + Send {
        T::list_key_envelopes(self, recipient_device_id, after_share_id)
    }
}

impl<T: ShareRepository> ShareRepository for std::sync::Arc<T> {
    fn find_share(
        &self,
        share_id: ShareId,
    ) -> impl Future<Output = Result<Option<ShareRecord>, RepositoryError>> + Send {
        T::find_share(self, share_id)
    }

    fn save_publication(
        &self,
        share: ShareRecord,
        snapshot: ShareSnapshotRecord,
        expected_revision: Option<u64>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::save_publication(self, share, snapshot, expected_revision)
    }

    fn find_snapshot(
        &self,
        share_id: ShareId,
        revision: u64,
    ) -> impl Future<Output = Result<Option<ShareSnapshotRecord>, RepositoryError>> + Send {
        T::find_snapshot(self, share_id, revision)
    }

    fn grant_share_access(
        &self,
        membership: ShareMembershipRecord,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::grant_share_access(self, membership)
    }

    fn revoke_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        T::revoke_share_access(self, share_id, user_id)
    }

    fn has_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        T::has_share_access(self, share_id, user_id)
    }

    fn list_authorized_shares(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<Vec<ShareRecord>, RepositoryError>> + Send {
        T::list_authorized_shares(self, user_id)
    }

    fn list_share_members(
        &self,
        share_id: ShareId,
    ) -> impl Future<Output = Result<Vec<UserId>, RepositoryError>> + Send {
        T::list_share_members(self, share_id)
    }
}

impl KeyEnvelopePage {
    /// Splits sorted records into the protocol's fixed-size response page.
    ///
    /// # Panics
    ///
    /// Panics if the protocol's fixed page size is zero when pagination is
    /// required. The protocol defines a non-zero page size.
    #[must_use]
    pub fn from_sorted(mut envelopes: Vec<KeyEnvelopeRecord>) -> Self {
        debug_assert!(
            envelopes
                .windows(2)
                .all(|pair| pair[0].share_id < pair[1].share_id)
        );
        let next_after_share_id = (envelopes.len() > MAX_KEY_ENVELOPES_PER_PAGE).then(|| {
            envelopes.truncate(MAX_KEY_ENVELOPES_PER_PAGE);
            envelopes
                .last()
                .expect("a non-empty page has a final cursor")
                .share_id
        });
        Self {
            envelopes,
            next_after_share_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_keeps_its_bound_and_returns_the_last_share_as_cursor() {
        let envelopes = (0..=MAX_KEY_ENVELOPES_PER_PAGE)
            .map(|index| {
                let index = u8::try_from(index).expect("test indices fit in a share ID byte");
                KeyEnvelopeRecord {
                    share_id: [index; SHARE_ID_BYTES],
                    recipient_device_id: [1; DEVICE_ID_BYTES],
                    ephemeral_public_key: [2; ENCRYPTION_KEY_BYTES],
                    ciphertext: vec![3],
                    created_at_unix_ns: 4,
                }
            })
            .collect();

        let page = KeyEnvelopePage::from_sorted(envelopes);
        let expected_cursor_byte = u8::try_from(MAX_KEY_ENVELOPES_PER_PAGE - 1)
            .expect("the test page size fits in a share ID byte");

        assert_eq!(page.envelopes.len(), MAX_KEY_ENVELOPES_PER_PAGE);
        assert_eq!(
            page.next_after_share_id,
            Some([expected_cursor_byte; SHARE_ID_BYTES])
        );
    }
}
