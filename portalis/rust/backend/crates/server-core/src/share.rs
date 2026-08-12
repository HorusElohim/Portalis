//! What publishing a snapshot does to a share.
//!
//! A share is the stable social object: a client-generated identifier, an
//! owner it never changes, and a revision that only ever moves forward. A
//! snapshot is immutable and content-addressed, so pointing at a different
//! `SnapshotId` is what "the share changed" means.
//!
//! These rules touch no storage and read no clock. Deciding what a
//! publication does, separately from performing it, is what lets the write
//! carry the revision it read and lose safely to a concurrent publisher.
//!
//! Nexus never sees inside a capsule. It compares capsule bytes to decide
//! whether a retry is identical, and otherwise treats them as opaque.

use portalis_nexus_protocol::{
    MAX_SHARE_CAPSULE_BYTES, MAX_SHARES_PER_RESPONSE, SIGNATURE_BYTES, SNAPSHOT_ID_BYTES,
};
use thiserror::Error;

use crate::ports::{
    Clock, RepositoryError, ShareId, ShareMembershipRecord, ShareRepository, ShareSnapshotRecord,
    UserDirectory, UserId,
};

/// The `BLAKE3` content root of a resolved canonical manifest.
pub type SnapshotId = [u8; SNAPSHOT_ID_BYTES];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ShareCommandError {
    #[error("capsule exceeds the {MAX_SHARE_CAPSULE_BYTES}-byte limit")]
    CapsuleTooLarge { actual: usize },
    #[error("capsule signature must contain exactly {SIGNATURE_BYTES} bytes")]
    InvalidSignatureLength { actual: usize },
    #[error("that share was not found")]
    NotFound,
    #[error("only the share owner may do that")]
    NotTheOwner,
    #[error("that member was not found")]
    UnknownMember,
    #[error("a share's owner cannot be removed from it")]
    OwnerCannotBeRemoved,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Whether `stored` is the very revision `publication` is trying to write.
///
/// A publisher whose acknowledgement was lost republishes the same bytes —
/// sealing is deterministic so that it can — and that is the revision already
/// stored rather than a competing one. Answering it without a write is what
/// keeps a lost reply from becoming a permanent failure. This judges nothing
/// about ordering; it recognises an exact repeat.
fn is_repeat(stored: &ShareRecord, publication: &Publication<'_>) -> bool {
    stored.revision == publication.revision
        && stored.snapshot_id == publication.snapshot_id
        && stored.capsule == publication.capsule
}

/// A share as it stands, with the snapshot its current revision points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareRecord {
    pub share_id: ShareId,
    /// Set by the first publication and never changed.
    pub owner: UserId,
    pub revision: u64,
    pub snapshot_id: SnapshotId,
    /// The encrypted snapshot capsule, opaque to Nexus.
    pub capsule: Vec<u8>,
    /// The owner's signature over the capsule, checked by recipients rather
    /// than by Nexus, which holds no key that could verify what is inside.
    pub capsule_signature: Vec<u8>,
    pub created_at_unix_ns: u64,
    pub updated_at_unix_ns: u64,
}

/// A signed request to move a share to its next revision.
#[derive(Clone, Copy, Debug)]
pub struct Publication<'a> {
    pub share_id: ShareId,
    pub publisher: UserId,
    pub revision: u64,
    /// The snapshot this publication was built from, absent for the first.
    pub snapshot_id: SnapshotId,
    pub capsule: &'a [u8],
    pub capsule_signature: &'a [u8],
}

/// Applies encrypted-share authorization and publication over durable state.
pub struct ShareService<S, C> {
    store: S,
    clock: C,
}

impl<S, C> ShareService<S, C>
where
    S: ShareRepository + UserDirectory,
    C: Clock,
{
    pub const fn new(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    /// Stores one immutable revision and advances the head, refusing only to
    /// overwrite a head this call did not see.
    ///
    /// The service no longer decides whether a revision is the right one.
    /// Under D3 a collection is a chain of signed revisions, so ordering,
    /// ownership and continuity are verified by whoever reads it, against the
    /// owner's device log and the revision they already hold — none of which
    /// the service has or should have. What is left here is a compare-and-set,
    /// which is an optimisation: it keeps two concurrent publishers from
    /// silently losing one another's work. A reader that is handed the loser
    /// anyway detects it, which is the point of the design.
    ///
    /// Signature verification belongs here too (`SPEC.md` §23 — storing
    /// garbage wastes space) and arrives with the revision itself, once the
    /// wire carries a self-verifying `Revision` rather than an opaque capsule
    /// and a detached signature it has no key to check.
    ///
    /// # Errors
    /// Returns [`ShareCommandError`] when the publication exceeds its bounds,
    /// loses the race twice, or cannot be persisted.
    pub async fn publish(
        &self,
        publication: Publication<'_>,
    ) -> Result<ShareRecord, ShareCommandError> {
        if publication.capsule.len() > MAX_SHARE_CAPSULE_BYTES {
            return Err(ShareCommandError::CapsuleTooLarge {
                actual: publication.capsule.len(),
            });
        }
        if publication.capsule_signature.len() != SIGNATURE_BYTES {
            return Err(ShareCommandError::InvalidSignatureLength {
                actual: publication.capsule_signature.len(),
            });
        }

        for _ in 0..2 {
            let current = self.store.find_share(publication.share_id).await?;
            // The precondition is only "the head is where I last saw it".
            // Whether the new revision follows the old one is the chain's
            // question, and it is asked by the reader.
            if let Some(stored) = current
                .as_ref()
                .filter(|stored| is_repeat(stored, &publication))
            {
                return Ok(stored.clone());
            }
            let expected = current.as_ref().map(|share| share.revision);
            let now = self.clock.now_unix_ns();
            let created = current
                .as_ref()
                .map_or(now, |share| share.created_at_unix_ns);
            let head = ShareRecord {
                share_id: publication.share_id,
                owner: publication.publisher,
                revision: publication.revision,
                snapshot_id: publication.snapshot_id,
                capsule: publication.capsule.to_vec(),
                capsule_signature: publication.capsule_signature.to_vec(),
                created_at_unix_ns: created,
                updated_at_unix_ns: now,
            };
            let snapshot = ShareSnapshotRecord {
                share_id: publication.share_id,
                revision: publication.revision,
                snapshot_id: publication.snapshot_id,
                capsule: publication.capsule.to_vec(),
                capsule_signature: publication.capsule_signature.to_vec(),
                created_at_unix_ns: now,
            };
            match self
                .store
                .save_publication(head.clone(), snapshot, expected)
                .await
            {
                Ok(()) => return Ok(head),
                Err(RepositoryError::VersionConflict) => {}
                Err(error) => return Err(error.into()),
            }
        }
        // Losing twice can still mean success, because the winner may have
        // published exactly these bytes. Anything else is reported rather than
        // retried forever: the publisher re-reads, rebuilds on what is now
        // current and signs again, which a revision naming its predecessor
        // obliges it to do anyway.
        let current = self.store.find_share(publication.share_id).await?;
        if let Some(stored) = current.filter(|stored| is_repeat(stored, &publication)) {
            return Ok(stored);
        }
        Err(RepositoryError::VersionConflict.into())
    }

    /// Fetches a share without revealing whether an unauthorized identifier
    /// exists: missing and private both return `NotFound`.
    ///
    /// # Errors
    /// Returns [`ShareCommandError::NotFound`] when inaccessible, or a storage error.
    pub async fn fetch(
        &self,
        user: UserId,
        share_id: ShareId,
    ) -> Result<ShareRecord, ShareCommandError> {
        if !self.store.has_share_access(share_id, user).await? {
            return Err(ShareCommandError::NotFound);
        }
        self.store
            .find_share(share_id)
            .await?
            .ok_or(ShareCommandError::NotFound)
    }

    /// # Errors
    /// Returns [`ShareCommandError`] when storage is unavailable.
    pub async fn list(&self, user: UserId) -> Result<Vec<ShareRecord>, ShareCommandError> {
        let mut shares = self.store.list_authorized_shares(user).await?;
        shares.sort_unstable_by_key(|share| share.share_id);
        shares.truncate(MAX_SHARES_PER_RESPONSE);
        Ok(shares)
    }

    /// # Errors
    /// Returns [`ShareCommandError`] when the share or member is absent, the
    /// actor is not its owner, or storage is unavailable.
    pub async fn grant(
        &self,
        owner: UserId,
        share_id: ShareId,
        member: UserId,
    ) -> Result<(), ShareCommandError> {
        let share = self
            .store
            .find_share(share_id)
            .await?
            .ok_or(ShareCommandError::NotFound)?;
        if share.owner != owner {
            return Err(ShareCommandError::NotTheOwner);
        }
        if self.store.find_user(member).await?.is_none() {
            return Err(ShareCommandError::UnknownMember);
        }
        self.store
            .grant_share_access(ShareMembershipRecord {
                share_id,
                user_id: member,
                granted_at_unix_ns: self.clock.now_unix_ns(),
            })
            .await?;
        Ok(())
    }

    /// Removes a member's access, or reports success when they had none.
    ///
    /// Revoking is what Nexus can do: it stops answering that user. The share
    /// key they already hold is beyond its reach, so an owner who means to
    /// exclude them rotates the key and publishes the next revision sealed
    /// only to the members who remain.
    ///
    /// # Errors
    /// Returns [`ShareCommandError`] when the share is absent, the actor is
    /// not its owner, the member named is the owner, or storage is
    /// unavailable.
    pub async fn revoke(
        &self,
        owner: UserId,
        share_id: ShareId,
        member: UserId,
    ) -> Result<(), ShareCommandError> {
        let share = self
            .store
            .find_share(share_id)
            .await?
            .ok_or(ShareCommandError::NotFound)?;
        if share.owner != owner {
            return Err(ShareCommandError::NotTheOwner);
        }
        // Ownership is not a membership edge, so removing it would delete
        // nothing while reporting success — a refusal is the honest answer.
        if member == share.owner {
            return Err(ShareCommandError::OwnerCannotBeRemoved);
        }
        self.store.revoke_share_access(share_id, member).await?;
        Ok(())
    }

    /// # Errors
    /// Returns [`ShareCommandError`] when storage is unavailable.
    pub async fn members(&self, share_id: ShareId) -> Result<Vec<UserId>, ShareCommandError> {
        Ok(self.store.list_share_members(share_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_700_000_000_000_000_000;
    const SHARE: ShareId = [1; 16];
    const OWNER: UserId = [2; 16];
    const STRANGER: UserId = [3; 16];
    const FIRST_SNAPSHOT: SnapshotId = [4; SNAPSHOT_ID_BYTES];
    const SECOND_SNAPSHOT: SnapshotId = [5; SNAPSHOT_ID_BYTES];

    fn user(id: UserId) -> crate::ports::UserRecord {
        crate::ports::UserRecord {
            user_id: id,
            username: format!("user{}", id[0]),
            normalized_username: format!("user{}", id[0]),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        }
    }

    fn signed_publication<'a>(
        publisher: UserId,
        revision: u64,
        snapshot_id: SnapshotId,
        capsule: &'a [u8],
        signature: &'a [u8; SIGNATURE_BYTES],
    ) -> Publication<'a> {
        Publication {
            share_id: SHARE,
            publisher,
            revision,
            snapshot_id,
            capsule,
            capsule_signature: signature,
        }
    }

    #[tokio::test]
    async fn authorized_members_fetch_while_strangers_cannot_probe_the_share() {
        use crate::memory::{FixedClock, InMemoryIdentities};

        let store = InMemoryIdentities::default();
        store.store_user(user(OWNER)).expect("owner");
        store.store_user(user(STRANGER)).expect("member");
        let service = ShareService::new(store, FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];
        let published = service
            .publish(signed_publication(
                OWNER,
                1,
                FIRST_SNAPSHOT,
                b"encrypted capsule",
                &signature,
            ))
            .await
            .expect("published");

        assert_eq!(service.fetch(OWNER, SHARE).await, Ok(published.clone()));
        assert_eq!(
            service.fetch(STRANGER, SHARE).await,
            Err(ShareCommandError::NotFound),
            "private existence is not disclosed"
        );

        service
            .grant(OWNER, SHARE, STRANGER)
            .await
            .expect("granted");
        assert_eq!(service.fetch(STRANGER, SHARE).await, Ok(published.clone()));

        // Both sides now list it, and the share knows who may read it.
        assert_eq!(service.list(OWNER).await, Ok(vec![published.clone()]));
        assert_eq!(service.list(STRANGER).await, Ok(vec![published.clone()]));
        let mut members = service.members(SHARE).await.expect("members");
        members.sort_unstable();
        assert_eq!(members, vec![OWNER, STRANGER]);

        // Revoking takes it all back: no fetch, no listing, no membership.
        assert_eq!(service.revoke(OWNER, SHARE, STRANGER).await, Ok(()));
        assert_eq!(
            service.fetch(STRANGER, SHARE).await,
            Err(ShareCommandError::NotFound)
        );
        assert_eq!(service.list(STRANGER).await, Ok(Vec::new()));
        assert_eq!(service.members(SHARE).await, Ok(vec![OWNER]));
        assert_eq!(
            service.fetch(OWNER, SHARE).await,
            Ok(published),
            "the owner still has their own share"
        );
    }

    /// Revoking is the inverse of granting, and every way of getting it wrong
    /// answers with something the caller can act on.
    #[tokio::test]
    async fn revoking_is_idempotent_and_refuses_what_it_cannot_do() {
        use crate::memory::{FixedClock, InMemoryIdentities};

        let store = InMemoryIdentities::default();
        store.store_user(user(OWNER)).expect("owner");
        store.store_user(user(STRANGER)).expect("member");
        let service = ShareService::new(store, FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];
        service
            .publish(signed_publication(
                OWNER,
                1,
                FIRST_SNAPSHOT,
                b"sealed",
                &signature,
            ))
            .await
            .expect("published");

        // A member who was never granted access is already absent, which is
        // the state the caller asked for.
        assert_eq!(service.revoke(OWNER, SHARE, STRANGER).await, Ok(()));

        service
            .grant(OWNER, SHARE, STRANGER)
            .await
            .expect("granted");
        assert_eq!(service.revoke(OWNER, SHARE, STRANGER).await, Ok(()));
        assert_eq!(
            service.revoke(OWNER, SHARE, STRANGER).await,
            Ok(()),
            "revoking twice is the same as revoking once"
        );

        // Ownership is not a membership edge, so removing it would delete
        // nothing while reporting success.
        assert_eq!(
            service.revoke(OWNER, SHARE, OWNER).await,
            Err(ShareCommandError::OwnerCannotBeRemoved)
        );
        assert_eq!(
            service.revoke(STRANGER, SHARE, OWNER).await,
            Err(ShareCommandError::NotTheOwner),
            "a member cannot remove anyone, least of all the owner"
        );
        assert_eq!(
            service.revoke(OWNER, [9; 16], STRANGER).await,
            Err(ShareCommandError::NotFound)
        );
    }

    #[tokio::test]
    async fn the_service_persists_immutable_history_and_never_regresses() {
        use crate::memory::{FixedClock, InMemoryIdentities};

        let service = ShareService::new(InMemoryIdentities::default(), FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];
        service
            .publish(signed_publication(
                OWNER,
                1,
                FIRST_SNAPSHOT,
                b"one",
                &signature,
            ))
            .await
            .expect("first");
        let advanced = service
            .publish(signed_publication(
                OWNER,
                2,
                SECOND_SNAPSHOT,
                b"two",
                &signature,
            ))
            .await
            .expect("second");

        assert_eq!(advanced.revision, 2);
        assert!(
            service
                .store
                .find_snapshot(SHARE, 1)
                .await
                .expect("read")
                .is_some()
        );
        // Republishing a number with different content is no longer a domain
        // refusal — the service does not judge revisions. It fails because
        // stored history is immutable, which is the store's invariant, and a
        // reader would see the same thing as a fork against what it holds.
        assert_eq!(
            service
                .publish(signed_publication(
                    OWNER,
                    1,
                    FIRST_SNAPSHOT,
                    b"different",
                    &signature,
                ))
                .await,
            Err(ShareCommandError::Repository(
                RepositoryError::VersionConflict
            ))
        );
    }

    #[tokio::test]
    async fn the_service_bounds_capsules_signatures_and_membership_changes() {
        use crate::memory::{FixedClock, InMemoryIdentities};

        let store = InMemoryIdentities::default();
        store.store_user(user(OWNER)).expect("owner");
        let service = ShareService::new(store, FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];
        let oversized = vec![0; MAX_SHARE_CAPSULE_BYTES + 1];
        assert_eq!(
            service
                .publish(signed_publication(
                    OWNER,
                    1,
                    FIRST_SNAPSHOT,
                    &oversized,
                    &signature,
                ))
                .await,
            Err(ShareCommandError::CapsuleTooLarge {
                actual: MAX_SHARE_CAPSULE_BYTES + 1
            })
        );
        let invalid_signature = Publication {
            capsule_signature: &[1; SIGNATURE_BYTES - 1],
            ..signed_publication(OWNER, 1, FIRST_SNAPSHOT, b"sealed", &signature)
        };
        assert!(matches!(
            service.publish(invalid_signature).await,
            Err(ShareCommandError::InvalidSignatureLength { .. })
        ));

        let publication = signed_publication(OWNER, 1, FIRST_SNAPSHOT, b"sealed", &signature);
        service.publish(publication).await.expect("published");
        assert_eq!(
            service
                .publish(publication)
                .await
                .expect("identical retry")
                .revision,
            1
        );
        assert_eq!(
            service.grant(STRANGER, SHARE, OWNER).await,
            Err(ShareCommandError::NotTheOwner)
        );
        assert_eq!(
            service.grant(OWNER, SHARE, STRANGER).await,
            Err(ShareCommandError::UnknownMember)
        );
        assert_eq!(
            service.grant(OWNER, [9; 16], OWNER).await,
            Err(ShareCommandError::NotFound)
        );
    }

    /// What a [`FailingStore`] does instead of working.
    enum Fault {
        /// Never lands a publication, optionally letting the publisher that
        /// beat it land after this many failed attempts.
        Publication {
            error: RepositoryError,
            install_after: Option<usize>,
        },
        /// Reads fine, but cannot record a membership.
        Grant(RepositoryError),
        /// Authorizes everyone, so a membership can outlive the share it
        /// names — the only way a permitted fetch finds nothing.
        PhantomAccess,
    }

    /// A store that answers everything normally except the one operation its
    /// fault names.
    ///
    /// `InMemoryIdentities` can only fail everything at once, which never
    /// reaches the write under test: the read before it fails first.
    struct FailingStore {
        inner: crate::memory::InMemoryIdentities,
        fault: Fault,
        attempts: std::sync::atomic::AtomicUsize,
    }

    impl FailingStore {
        fn losing_race(install_after: Option<usize>) -> Self {
            Self::new(Fault::Publication {
                error: RepositoryError::VersionConflict,
                install_after,
            })
        }

        fn new(fault: Fault) -> Self {
            Self {
                inner: crate::memory::InMemoryIdentities::default(),
                fault,
                attempts: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl UserDirectory for FailingStore {
        async fn find_user(
            &self,
            user_id: UserId,
        ) -> Result<Option<crate::UserRecord>, RepositoryError> {
            self.inner.find_user(user_id).await
        }

        async fn find_user_by_handle(
            &self,
            normalized_username: &str,
            discriminator: &str,
        ) -> Result<Option<crate::UserRecord>, RepositoryError> {
            self.inner
                .find_user_by_handle(normalized_username, discriminator)
                .await
        }
    }

    impl ShareRepository for FailingStore {
        async fn find_share(
            &self,
            share_id: ShareId,
        ) -> Result<Option<ShareRecord>, RepositoryError> {
            self.inner.find_share(share_id).await
        }

        /// Under [`Fault::Publication`] this never succeeds, and on the
        /// configured attempt it first writes the publication through,
        /// standing in for the device that won.
        async fn save_publication(
            &self,
            share: ShareRecord,
            snapshot: ShareSnapshotRecord,
            expected_revision: Option<u64>,
        ) -> Result<(), RepositoryError> {
            let Fault::Publication {
                error,
                install_after,
            } = &self.fault
            else {
                return self
                    .inner
                    .save_publication(share, snapshot, expected_revision)
                    .await;
            };
            let attempt = self
                .attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            if *install_after == Some(attempt) {
                self.inner.save_publication(share, snapshot, None).await?;
            }
            Err(error.clone())
        }

        async fn find_snapshot(
            &self,
            share_id: ShareId,
            revision: u64,
        ) -> Result<Option<ShareSnapshotRecord>, RepositoryError> {
            self.inner.find_snapshot(share_id, revision).await
        }

        async fn grant_share_access(
            &self,
            membership: ShareMembershipRecord,
        ) -> Result<(), RepositoryError> {
            match &self.fault {
                Fault::Grant(error) => Err(error.clone()),
                _ => self.inner.grant_share_access(membership).await,
            }
        }

        async fn revoke_share_access(
            &self,
            share_id: ShareId,
            user_id: UserId,
        ) -> Result<(), RepositoryError> {
            match &self.fault {
                Fault::Grant(error) => Err(error.clone()),
                _ => self.inner.revoke_share_access(share_id, user_id).await,
            }
        }

        async fn has_share_access(
            &self,
            share_id: ShareId,
            user_id: UserId,
        ) -> Result<bool, RepositoryError> {
            match &self.fault {
                Fault::PhantomAccess => Ok(true),
                _ => self.inner.has_share_access(share_id, user_id).await,
            }
        }

        async fn list_authorized_shares(
            &self,
            user_id: UserId,
        ) -> Result<Vec<ShareRecord>, RepositoryError> {
            self.inner.list_authorized_shares(user_id).await
        }

        async fn list_share_members(
            &self,
            share_id: ShareId,
        ) -> Result<Vec<UserId>, RepositoryError> {
            self.inner.list_share_members(share_id).await
        }
    }

    /// Losing to a device that published the very same bytes is success: both
    /// devices wanted the same revision, and one of them achieved it.
    #[tokio::test]
    async fn losing_to_an_identical_publication_still_succeeds() {
        use crate::memory::FixedClock;

        // Installed on the last attempt, so both loop iterations lose and the
        // final re-read is what discovers the winner.
        let service = ShareService::new(FailingStore::losing_race(Some(2)), FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];

        let published = service
            .publish(signed_publication(
                OWNER,
                1,
                FIRST_SNAPSHOT,
                b"sealed",
                &signature,
            ))
            .await
            .expect("the winner published exactly what this publisher wanted");

        assert_eq!(published.revision, 1);
        assert_eq!(published.capsule, b"sealed".to_vec());

        // The share the winner created is a real one: its owner may grant
        // access to it like any other.
        service
            .store
            .inner
            .store_user(user(STRANGER))
            .expect("member");
        assert_eq!(service.grant(OWNER, SHARE, STRANGER).await, Ok(()));
        assert_eq!(
            service.store.has_share_access(SHARE, STRANGER).await,
            Ok(true)
        );
        assert_eq!(service.revoke(OWNER, SHARE, STRANGER).await, Ok(()));
        assert_eq!(
            service.store.has_share_access(SHARE, STRANGER).await,
            Ok(false),
            "and revoked again like any other share"
        );
    }

    /// A share nobody managed to move reports contention rather than
    /// pretending the publication landed.
    #[tokio::test]
    async fn exhausting_the_retries_reports_the_lost_race() {
        use crate::memory::FixedClock;

        let service = ShareService::new(FailingStore::losing_race(None), FixedClock::new(NOW));
        let signature = [8; SIGNATURE_BYTES];

        assert_eq!(
            service
                .publish(signed_publication(
                    OWNER,
                    1,
                    FIRST_SNAPSHOT,
                    b"sealed",
                    &signature,
                ))
                .await,
            Err(ShareCommandError::Repository(
                RepositoryError::VersionConflict
            ))
        );

        // The share does not exist as far as any reader is concerned.
        assert_eq!(
            service.fetch(OWNER, SHARE).await,
            Err(ShareCommandError::NotFound)
        );
        assert_eq!(service.list(OWNER).await, Ok(Vec::new()));
        assert_eq!(service.members(SHARE).await, Ok(Vec::new()));
        assert_eq!(
            service.grant(OWNER, SHARE, STRANGER).await,
            Err(ShareCommandError::NotFound)
        );
        assert_eq!(
            service.revoke(OWNER, SHARE, STRANGER).await,
            Err(ShareCommandError::NotFound)
        );

        // Nothing was left behind by the attempts that lost: no head, no
        // history row, no membership, and no user invented along the way.
        let store = &service.store;
        assert_eq!(store.find_share(SHARE).await, Ok(None));
        assert_eq!(store.find_snapshot(SHARE, 1).await, Ok(None));
        assert_eq!(store.has_share_access(SHARE, OWNER).await, Ok(false));
        assert_eq!(store.list_authorized_shares(OWNER).await, Ok(Vec::new()));
        assert_eq!(store.list_share_members(SHARE).await, Ok(Vec::new()));
        assert_eq!(store.find_user(OWNER).await, Ok(None));
        assert_eq!(store.find_user_by_handle("ada", "7Q2XZ").await, Ok(None));
    }

    /// A store that fails for any other reason is an outage to report, not a
    /// race to retry: repeating the write would not change the answer.
    #[tokio::test]
    async fn a_storage_failure_while_publishing_is_reported_rather_than_retried() {
        use crate::memory::FixedClock;

        let service = ShareService::new(
            FailingStore::new(Fault::Publication {
                error: RepositoryError::Unavailable("disk".to_owned()),
                install_after: None,
            }),
            FixedClock::new(NOW),
        );
        let signature = [8; SIGNATURE_BYTES];

        assert_eq!(
            service
                .publish(signed_publication(
                    OWNER,
                    1,
                    FIRST_SNAPSHOT,
                    b"sealed",
                    &signature,
                ))
                .await,
            Err(ShareCommandError::Repository(RepositoryError::Unavailable(
                "disk".to_owned()
            )))
        );
    }

    /// Authorization and existence are two separate reads, so a membership
    /// that outlived its share must report the share missing rather than
    /// answer with nothing and call it success.
    #[tokio::test]
    async fn a_permitted_fetch_of_a_share_that_is_gone_reports_it_missing() {
        use crate::memory::FixedClock;

        let service = ShareService::new(
            FailingStore::new(Fault::PhantomAccess),
            FixedClock::new(NOW),
        );

        assert_eq!(
            service.fetch(STRANGER, SHARE).await,
            Err(ShareCommandError::NotFound)
        );
    }

    /// Granting reads the share and the member before it writes, so a store
    /// that fails at the write reports the outage rather than a refusal the
    /// owner could act on.
    #[tokio::test]
    async fn a_storage_failure_while_granting_is_reported() {
        use crate::memory::FixedClock;

        let service = ShareService::new(
            FailingStore::new(Fault::Grant(RepositoryError::Unavailable(
                "disk".to_owned(),
            ))),
            FixedClock::new(NOW),
        );
        let signature = [8; SIGNATURE_BYTES];
        service
            .store
            .inner
            .store_user(user(STRANGER))
            .expect("member");
        service
            .publish(signed_publication(
                OWNER,
                1,
                FIRST_SNAPSHOT,
                b"sealed",
                &signature,
            ))
            .await
            .expect("published");

        assert_eq!(
            service.grant(OWNER, SHARE, STRANGER).await,
            Err(ShareCommandError::Repository(RepositoryError::Unavailable(
                "disk".to_owned()
            )))
        );
        assert_eq!(
            service.revoke(OWNER, SHARE, STRANGER).await,
            Err(ShareCommandError::Repository(RepositoryError::Unavailable(
                "disk".to_owned()
            ))),
            "a revocation that did not happen is not reported as done"
        );
    }
}
