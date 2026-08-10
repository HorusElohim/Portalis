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

use portalis_nexus_protocol::SNAPSHOT_ID_BYTES;
use thiserror::Error;

use crate::ports::{ShareId, UserId};

/// The `BLAKE3` content root of a resolved canonical manifest.
pub type SnapshotId = [u8; SNAPSHOT_ID_BYTES];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ShareError {
    /// Ownership is permanent, so a peer that may seed and fetch a share
    /// still cannot publish over it.
    #[error("that share belongs to someone else")]
    NotTheOwner,
    /// A share that does not exist yet starts at revision one. Anything else
    /// means the publisher is working from state this server never had.
    #[error("a new share starts at revision 1, not {actual}")]
    MustStartAtOne { actual: u64 },
    /// Revisions never regress and never skip: the next one is always
    /// exactly one past what is stored.
    #[error("expected revision {expected}, got {actual}")]
    OutOfOrder { expected: u64, actual: u64 },
    /// The publication names a prior snapshot the share is not on, so it was
    /// built from a revision someone else has already moved past.
    #[error("this publication follows a snapshot the share has moved past")]
    PriorSnapshotMismatch,
    /// The stored revision was published with different content. Revisions
    /// are immutable once written; the publisher takes the next one.
    #[error("revision {revision} was already published with different content")]
    Conflict { revision: u64 },
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
    pub prior_snapshot_id: Option<SnapshotId>,
    pub snapshot_id: SnapshotId,
    pub capsule: &'a [u8],
    pub capsule_signature: &'a [u8],
}

/// What publishing should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publish {
    /// The share does not exist. The write must find no row, which is what
    /// makes the first publisher the permanent owner even under a race.
    Create,
    /// The share moves forward, and the write must still find
    /// `expected_revision` stored.
    Advance { expected_revision: u64 },
    /// A byte-identical retry of the revision already stored. Publishing is
    /// idempotent, so this reports success without writing.
    Unchanged,
}

/// Decides what `publication` does to `current`.
///
/// Passing `None` for `current` means no share exists under that identifier
/// yet, and this publication would create it.
///
/// # Errors
///
/// Returns [`ShareError`] when the publisher does not own the share, the
/// revision does not follow the stored one, the publication was built from a
/// snapshot the share has moved past, or the stored revision holds different
/// content.
pub fn publish(
    current: Option<&ShareRecord>,
    publication: &Publication<'_>,
) -> Result<Publish, ShareError> {
    let Some(current) = current else {
        return first(publication);
    };

    if current.owner != publication.publisher {
        return Err(ShareError::NotTheOwner);
    }
    if publication.revision == current.revision {
        return retry(current, publication);
    }
    if publication.revision != current.revision + 1 {
        return Err(ShareError::OutOfOrder {
            expected: current.revision + 1,
            actual: publication.revision,
        });
    }
    // The publication must follow the snapshot the share is actually on, or
    // it was built against a revision another device already replaced.
    if publication.prior_snapshot_id != Some(current.snapshot_id) {
        return Err(ShareError::PriorSnapshotMismatch);
    }
    Ok(Publish::Advance {
        expected_revision: current.revision,
    })
}

/// The first publication creates the share and fixes its owner.
fn first(publication: &Publication<'_>) -> Result<Publish, ShareError> {
    if publication.revision != 1 {
        return Err(ShareError::MustStartAtOne {
            actual: publication.revision,
        });
    }
    // Nothing to follow, so naming a prior snapshot means this was built
    // against a share this server has no record of.
    if publication.prior_snapshot_id.is_some() {
        return Err(ShareError::PriorSnapshotMismatch);
    }
    Ok(Publish::Create)
}

/// Republishing the stored revision succeeds only if nothing about it moved.
fn retry(current: &ShareRecord, publication: &Publication<'_>) -> Result<Publish, ShareError> {
    let identical = current.snapshot_id == publication.snapshot_id
        && current.capsule == publication.capsule
        && current.capsule_signature == publication.capsule_signature;
    if identical {
        Ok(Publish::Unchanged)
    } else {
        Err(ShareError::Conflict {
            revision: current.revision,
        })
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

    fn stored(revision: u64, snapshot_id: SnapshotId) -> ShareRecord {
        ShareRecord {
            share_id: SHARE,
            owner: OWNER,
            revision,
            snapshot_id,
            capsule: b"sealed".to_vec(),
            capsule_signature: b"signature".to_vec(),
            created_at_unix_ns: NOW,
            updated_at_unix_ns: NOW,
        }
    }

    fn publication(revision: u64) -> Publication<'static> {
        Publication {
            share_id: SHARE,
            publisher: OWNER,
            revision,
            prior_snapshot_id: None,
            snapshot_id: FIRST_SNAPSHOT,
            capsule: b"sealed",
            capsule_signature: b"signature",
        }
    }

    #[test]
    fn the_first_publication_creates_the_share() {
        assert_eq!(publish(None, &publication(1)), Ok(Publish::Create));
    }

    #[test]
    fn a_new_share_cannot_start_partway_through() {
        // A publisher holding revision 7 for a share this server never saw is
        // working from state that is not ours; creating it would invent a
        // history nobody published.
        assert_eq!(
            publish(None, &publication(7)),
            Err(ShareError::MustStartAtOne { actual: 7 })
        );
        assert_eq!(
            publish(None, &publication(0)),
            Err(ShareError::MustStartAtOne { actual: 0 })
        );
    }

    #[test]
    fn a_first_publication_has_nothing_to_follow() {
        let built_on_something_else = Publication {
            prior_snapshot_id: Some(FIRST_SNAPSHOT),
            ..publication(1)
        };

        assert_eq!(
            publish(None, &built_on_something_else),
            Err(ShareError::PriorSnapshotMismatch)
        );
    }

    #[test]
    fn only_the_owner_may_publish() {
        let peer = Publication {
            publisher: STRANGER,
            ..publication(2)
        };

        assert_eq!(
            publish(Some(&stored(1, FIRST_SNAPSHOT)), &peer),
            Err(ShareError::NotTheOwner),
            "a peer may seed and fetch a share without being able to move it"
        );
    }

    #[test]
    fn a_share_advances_one_revision_at_a_time() {
        let next = Publication {
            revision: 2,
            prior_snapshot_id: Some(FIRST_SNAPSHOT),
            snapshot_id: SECOND_SNAPSHOT,
            ..publication(2)
        };

        assert_eq!(
            publish(Some(&stored(1, FIRST_SNAPSHOT)), &next),
            Ok(Publish::Advance {
                expected_revision: 1
            })
        );
    }

    #[test]
    fn revisions_never_regress_or_skip() {
        let current = stored(4, FIRST_SNAPSHOT);

        for (revision, expected) in [
            (3, 5),
            (1, 5),
            (0, 5),
            // Skipping ahead is the same failure from the other side: the
            // publisher is not working from what is stored.
            (6, 5),
            (99, 5),
        ] {
            let attempt = Publication {
                revision,
                prior_snapshot_id: Some(FIRST_SNAPSHOT),
                snapshot_id: SECOND_SNAPSHOT,
                ..publication(revision)
            };

            assert_eq!(
                publish(Some(&current), &attempt),
                Err(ShareError::OutOfOrder {
                    expected,
                    actual: revision
                }),
                "revision {revision}"
            );
        }
    }

    #[test]
    fn an_advance_must_follow_the_snapshot_the_share_is_on() {
        let built_on_a_replaced_snapshot = Publication {
            revision: 2,
            prior_snapshot_id: Some(SECOND_SNAPSHOT),
            snapshot_id: [9; SNAPSHOT_ID_BYTES],
            ..publication(2)
        };

        assert_eq!(
            publish(
                Some(&stored(1, FIRST_SNAPSHOT)),
                &built_on_a_replaced_snapshot
            ),
            Err(ShareError::PriorSnapshotMismatch)
        );

        let naming_nothing = Publication {
            revision: 2,
            prior_snapshot_id: None,
            ..publication(2)
        };
        assert_eq!(
            publish(Some(&stored(1, FIRST_SNAPSHOT)), &naming_nothing),
            Err(ShareError::PriorSnapshotMismatch)
        );
    }

    /// A publisher that loses its answer retries the same bytes. That must
    /// succeed without writing, or a dropped response would strand a device
    /// unable to move forward or repeat itself.
    #[test]
    fn an_identical_retry_changes_nothing() {
        assert_eq!(
            publish(Some(&stored(1, FIRST_SNAPSHOT)), &publication(1)),
            Ok(Publish::Unchanged)
        );
    }

    #[test]
    fn republishing_a_revision_with_different_content_is_refused() {
        let current = stored(1, FIRST_SNAPSHOT);

        for altered in [
            Publication {
                snapshot_id: SECOND_SNAPSHOT,
                ..publication(1)
            },
            Publication {
                capsule: b"different",
                ..publication(1)
            },
            Publication {
                capsule_signature: b"different",
                ..publication(1)
            },
        ] {
            assert_eq!(
                publish(Some(&current), &altered),
                Err(ShareError::Conflict { revision: 1 }),
                "a published revision is immutable"
            );
        }
    }
}
