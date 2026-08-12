//! Deciding whether a revision belongs after the one already held.
//!
//! `protocol` can say whether a revision is well-formed and whether the key it
//! names signed it. Neither answers the question that matters, because both
//! dangerous attacks pass them. A revision signed by a device the owner
//! revoked last week is perfectly signed. An older revision, served in place
//! of the current one to undo a removal, is perfectly valid. A fork — two
//! revisions with the same number, both properly signed — is two perfectly
//! valid objects.
//!
//! What separates them is outside knowledge: the owner's device log, and the
//! highest revision this device already verified. That is why verification
//! lives in `client` and never in the service. The service stores and
//! forwards; a reader decides.
//!
//! Verification follows `SPEC.md` §7.3 in order — signature, then author
//! authority, then position in the chain, then the manifest it names — because
//! each step is more expensive than the last and a caller should not pay for
//! a store read to reject a forged signature.
//!
//! **Forks are never resolved silently.** A fork means a compromised owner
//! device or a service splitting members' views. The first revision seen at a
//! number is kept, the second is refused, and the caller is told, because
//! choosing between them is not a decision code can make correctly.

use portalis_nexus_protocol::{
    DEVICE_KEY_BYTES, DeviceLog, ManifestHash, Revision, RevisionError, RevisionHash,
    SHARE_ID_BYTES,
};
use thiserror::Error;

/// The highest revision of one collection this device has verified.
///
/// Holding it is what makes a rollback detectable: without it, an older
/// revision is indistinguishable from a current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainState {
    pub collection_id: [u8; SHARE_ID_BYTES],
    pub number: u64,
    /// The revision's own hash, so a second revision at the same number is
    /// recognised as a fork rather than accepted as a repeat.
    pub revision_hash: RevisionHash,
}

/// Where the highest verified revision per collection is kept.
///
/// A port rather than a concrete store, because step 6 replaces the in-memory
/// implementation with the local database and nothing here should change when
/// it does. Reads return `None` for a collection never seen, which is how a
/// first revision is told apart from a rollback.
pub trait ChainStore: Send + Sync {
    /// The highest verified revision of `collection`, if any.
    fn highest(
        &self,
        collection: [u8; SHARE_ID_BYTES],
    ) -> impl std::future::Future<Output = Result<Option<ChainState>, ChainStoreError>> + Send;

    /// Records a newly verified revision as the highest.
    ///
    /// Called only after verification succeeds, so an implementation may
    /// assume monotonicity and does not re-check it.
    fn record(
        &self,
        state: ChainState,
    ) -> impl std::future::Future<Output = Result<(), ChainStoreError>> + Send;
}

/// A store that could not answer. Distinct from a refusal: nothing was
/// decided, so a caller retries rather than treating the revision as hostile.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("the chain store is unavailable: {reason}")]
pub struct ChainStoreError {
    pub reason: String,
}

impl ChainStoreError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Why a revision is not accepted.
///
/// Each variant is one attack from `SPEC.md` §22 with the outcome §18
/// promises, because a single "invalid revision" would let a rollback and a
/// forged signature look alike to a user and to a test.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ChainError {
    /// Shown as "Cannot verify".
    #[error("the revision is not well-formed: {0}")]
    Malformed(#[from] RevisionError),
    /// Shown as "Cannot verify".
    #[error("the signature on revision {number} is not the author's")]
    ForgedSignature { number: u64 },
    /// The revision claims a different owner than the log it was verified
    /// against. Shown as "Cannot verify".
    #[error("revision {number} names an owner this device log does not belong to")]
    NotTheOwner { number: u64 },
    /// Shown as "Cannot verify".
    #[error("revision {number} is signed by a device the owner never enrolled")]
    UnknownAuthor { number: u64 },
    /// The attack a device log exists to catch: a signature from a device
    /// whose authority the owner ended. Shown as "Cannot verify".
    #[error("revision {number} is signed by a device the owner revoked")]
    RevokedAuthor { number: u64 },
    /// A collection whose first revision is not numbered one, which means
    /// history is being started midway.
    #[error("revision {number} is the first seen for this collection, and a chain begins at 1")]
    NotTheFirst { number: u64 },
    /// Nothing forged: an older genuine revision offered as the current one.
    /// Shown as "Cannot verify".
    #[error("revision {offered} is behind the {held} already verified")]
    Rollback { held: u64, offered: u64 },
    /// Two valid revisions with one number. Shown as "Conflicting history —
    /// needs attention", and never resolved silently.
    #[error(
        "revision {number} conflicts with a different revision already verified at that number"
    )]
    Fork {
        number: u64,
        kept: RevisionHash,
        refused: RevisionHash,
    },
    /// Right number, wrong ancestor.
    #[error("revision {number} does not follow the revision already verified")]
    ChainBroken { number: u64 },
    #[error("revision {actual} skips {expected}, and a chain has no gaps")]
    SequenceGap { expected: u64, actual: u64 },
    /// The manifest fetched is not the one the revision signed for.
    #[error("revision {number} names a manifest other than the one fetched")]
    ManifestMismatch { number: u64 },
    #[error(transparent)]
    Store(#[from] ChainStoreError),
}

/// A revision that verified, and what the caller should do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accepted {
    /// The new highest state, already recorded.
    pub state: ChainState,
    /// Members whose device log has moved since the owner sealed to them.
    ///
    /// Not a failure. It means those members have linked or revoked a device
    /// since, so the content key needs sealing again — the difference between
    /// "a new device opens nothing" being a mystery and being a known state.
    pub reseal_owed: Vec<[u8; DEVICE_KEY_BYTES]>,
}

/// Verifies a revision against the owner's device log and what is already
/// held, and records it when it passes.
///
/// `manifest_hash` is the hash of the manifest actually fetched, checked last
/// because fetching it is the most expensive thing here. Pass `None` when the
/// manifest has not been fetched yet; the check is then the caller's to make
/// before trusting a byte of it.
///
/// `member_logs` supplies each member's current device log hash where known,
/// and only feeds [`Accepted::reseal_owed`]. A member absent from it is not
/// reported, because an unknown log state is not a stale one.
///
/// # Errors
///
/// Returns the first [`ChainError`] the revision breaks, in §7.3's order.
pub async fn verify<S: ChainStore>(
    revision: &Revision,
    owner_log: &DeviceLog,
    store: &S,
    manifest_hash: Option<ManifestHash>,
    member_logs: &[([u8; DEVICE_KEY_BYTES], portalis_nexus_protocol::LogHash)],
) -> Result<Accepted, ChainError> {
    revision.validate()?;

    // Cheapest first, and self-contained: no store read pays for a forgery.
    if !revision.verify() {
        return Err(ChainError::ForgedSignature {
            number: revision.number,
        });
    }

    // The log must be the one this revision claims to answer to. Verifying
    // against some other person's log would prove nothing about this owner.
    if revision.owner_root_key != owner_log.root_key() {
        return Err(ChainError::NotTheOwner {
            number: revision.number,
        });
    }
    match owner_log
        .history()
        .into_iter()
        .find(|device| device.signing_key == revision.author_key)
    {
        None => {
            return Err(ChainError::UnknownAuthor {
                number: revision.number,
            });
        }
        Some(device) if !device.is_authorized() => {
            return Err(ChainError::RevokedAuthor {
                number: revision.number,
            });
        }
        Some(_) => {}
    }

    let held = store.highest(revision.collection_id).await?;
    position(revision, held.as_ref())?;

    if let Some(expected) = manifest_hash
        && expected != revision.manifest_hash
    {
        return Err(ChainError::ManifestMismatch {
            number: revision.number,
        });
    }

    let state = ChainState {
        collection_id: revision.collection_id,
        number: revision.number,
        revision_hash: revision.hash(),
    };
    store.record(state).await?;

    Ok(Accepted {
        state,
        reseal_owed: reseal_owed(revision, member_logs),
    })
}

/// Whether this revision belongs where it claims to.
///
/// Split out because it is the part with no cryptography in it: given what is
/// held, a number and a previous hash either follow or they do not.
fn position(revision: &Revision, held: Option<&ChainState>) -> Result<(), ChainError> {
    let Some(held) = held else {
        // Nothing held: only a genuine beginning is acceptable, or a reader
        // could be handed a chain from the middle and never know what it
        // missed.
        return if revision.number == 1 {
            Ok(())
        } else {
            Err(ChainError::NotTheFirst {
                number: revision.number,
            })
        };
    };

    if revision.number == held.number {
        // The same revision again is where we already are. A different one at
        // the same number is a fork, and the first seen is the one kept.
        return if revision.hash() == held.revision_hash {
            Ok(())
        } else {
            Err(ChainError::Fork {
                number: revision.number,
                kept: held.revision_hash,
                refused: revision.hash(),
            })
        };
    }
    if revision.number < held.number {
        return Err(ChainError::Rollback {
            held: held.number,
            offered: revision.number,
        });
    }
    if revision.number != held.number + 1 {
        return Err(ChainError::SequenceGap {
            expected: held.number + 1,
            actual: revision.number,
        });
    }
    if revision.previous_hash != held.revision_hash {
        return Err(ChainError::ChainBroken {
            number: revision.number,
        });
    }
    Ok(())
}

/// Members whose device log has moved since the owner sealed to them.
fn reseal_owed(
    revision: &Revision,
    member_logs: &[([u8; DEVICE_KEY_BYTES], portalis_nexus_protocol::LogHash)],
) -> Vec<[u8; DEVICE_KEY_BYTES]> {
    member_logs
        .iter()
        .filter(|(member, current)| {
            revision
                .sealed_against(member)
                .is_some_and(|sealed| &sealed != current)
        })
        .map(|(member, _)| *member)
        .collect()
}

/// A [`ChainStore`] in memory, for tests, demos, and any caller that has not
/// got a database yet. Step 6 replaces it without changing a signature.
#[derive(Debug, Default)]
pub struct MemoryChainStore {
    highest: std::sync::Mutex<std::collections::HashMap<[u8; SHARE_ID_BYTES], ChainState>>,
}

impl ChainStore for MemoryChainStore {
    fn highest(
        &self,
        collection: [u8; SHARE_ID_BYTES],
    ) -> impl std::future::Future<Output = Result<Option<ChainState>, ChainStoreError>> + Send {
        let held = self.lock().get(&collection).copied();
        async move { Ok(held) }
    }

    fn record(
        &self,
        state: ChainState,
    ) -> impl std::future::Future<Output = Result<(), ChainStoreError>> + Send {
        self.lock().insert(state.collection_id, state);
        async move { Ok(()) }
    }
}

impl MemoryChainStore {
    fn lock(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<[u8; SHARE_ID_BYTES], ChainState>>
    {
        self.highest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    //! Every test here is an attack that passes `protocol`'s checks. That is
    //! the point of the module: a revision can be perfectly formed, perfectly
    //! signed, and still be a lie about what happened.

    use ed25519_dalek::{Signer, SigningKey};
    use portalis_nexus_protocol::{
        Action, ENCRYPTION_KEY_BYTES, LogEntry, LogHash, Member, NO_PREVIOUS_ENTRY,
        NO_PREVIOUS_REVISION, REVISION_HASH_BYTES, SIGNATURE_BYTES,
    };

    use super::*;

    const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
    const OWNER_SEED: [u8; 32] = [1; 32];
    const SECOND_SEED: [u8; 32] = [2; 32];
    const STRANGER_SEED: [u8; 32] = [9; 32];
    const MANIFEST: ManifestHash = [0x22; 32];
    const NOW: u64 = 1_700_000_000_000_000_000;

    fn key(seed: [u8; 32]) -> SigningKey {
        SigningKey::from_bytes(&seed)
    }

    fn public(signer: &SigningKey) -> [u8; DEVICE_KEY_BYTES] {
        signer.verifying_key().to_bytes()
    }

    /// An owner with two devices: the root, and a second it enrolled.
    fn owner_log(owner: &SigningKey, second: &SigningKey) -> (DeviceLog, Vec<LogEntry>) {
        let root = sign_entry(
            LogEntry {
                root_key: public(owner),
                sequence: 1,
                previous_hash: NO_PREVIOUS_ENTRY,
                action: Action::Enrol,
                subject_signing_key: public(owner),
                subject_encryption_key: [0x40; ENCRYPTION_KEY_BYTES],
                at_unix_ns: NOW,
                author_key: public(owner),
                signature: [0; SIGNATURE_BYTES],
            },
            owner,
        );
        let enrol = sign_entry(
            LogEntry {
                root_key: public(owner),
                sequence: 2,
                previous_hash: root.hash(),
                action: Action::Enrol,
                subject_signing_key: public(second),
                subject_encryption_key: [0x41; ENCRYPTION_KEY_BYTES],
                at_unix_ns: NOW + 1,
                author_key: public(owner),
                signature: [0; SIGNATURE_BYTES],
            },
            owner,
        );
        let entries = vec![root, enrol];
        (
            DeviceLog::replay(&entries).expect("the owner's own devices"),
            entries,
        )
    }

    fn sign_entry(mut entry: LogEntry, author: &SigningKey) -> LogEntry {
        entry.signature = author.sign(&entry.signing_payload()).to_bytes();
        entry
    }

    /// A store that fails on demand, so the difference between "refused" and
    /// "could not decide" is exercised rather than assumed.
    #[derive(Default)]
    struct Broken {
        fail_read: bool,
    }

    impl ChainStore for Broken {
        fn highest(
            &self,
            _collection: [u8; SHARE_ID_BYTES],
        ) -> impl std::future::Future<Output = Result<Option<ChainState>, ChainStoreError>> + Send
        {
            let fail = self.fail_read;
            async move {
                if fail {
                    Err(ChainStoreError::new("the disk is gone"))
                } else {
                    Ok(None)
                }
            }
        }

        async fn record(&self, _state: ChainState) -> Result<(), ChainStoreError> {
            Err(ChainStoreError::new("the disk is full"))
        }
    }

    fn member(root: u8, log_hash: u8) -> Member {
        Member {
            root_key: [root; DEVICE_KEY_BYTES],
            device_log_hash: [log_hash; 32],
        }
    }

    fn revision(owner: &SigningKey, number: u64, previous: RevisionHash) -> Revision {
        sign_revision(
            Revision {
                collection_id: COLLECTION,
                number,
                previous_hash: previous,
                manifest_hash: MANIFEST,
                owner_root_key: public(owner),
                at_unix_ns: NOW + number,
                members: vec![member(2, 0x80), member(3, 0x81)],
                author_key: public(owner),
                signature: [0; SIGNATURE_BYTES],
            },
            owner,
        )
    }

    fn sign_revision(mut revision: Revision, author: &SigningKey) -> Revision {
        revision.signature = author.sign(&revision.signing_payload()).to_bytes();
        revision
    }

    async fn accept(
        revision: &Revision,
        log: &DeviceLog,
        store: &MemoryChainStore,
    ) -> Result<Accepted, ChainError> {
        verify(revision, log, store, Some(MANIFEST), &[]).await
    }

    #[tokio::test]
    async fn a_chain_is_accepted_one_revision_at_a_time() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);
        let accepted = accept(&first, &log, &store).await.expect("revision 1");
        assert_eq!(accepted.state.number, 1);
        assert_eq!(accepted.state.revision_hash, first.hash());
        assert!(accepted.reseal_owed.is_empty());

        let second_revision = revision(&owner, 2, first.hash());
        let accepted = accept(&second_revision, &log, &store)
            .await
            .expect("revision 2");
        assert_eq!(accepted.state.number, 2);

        let third = revision(&owner, 3, second_revision.hash());
        assert_eq!(
            accept(&third, &log, &store)
                .await
                .expect("revision 3")
                .state
                .number,
            3
        );
    }

    /// Any enrolled owner device may publish, not only the root.
    #[tokio::test]
    async fn a_second_owner_device_may_publish() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let mut first = revision(&owner, 1, NO_PREVIOUS_REVISION);
        first.author_key = public(&second);
        let first = sign_revision(first, &second);

        assert_eq!(
            accept(&first, &log, &store)
                .await
                .expect("revision 1")
                .state
                .number,
            1
        );
    }

    #[tokio::test]
    async fn re_offering_the_revision_already_held_is_accepted_as_where_we_are() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();
        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);

        accept(&first, &log, &store).await.expect("revision 1");

        assert_eq!(
            accept(&first, &log, &store)
                .await
                .expect("the same revision again")
                .state
                .revision_hash,
            first.hash()
        );
    }

    /// The attack a device log exists to catch, at the revision layer.
    #[tokio::test]
    async fn a_revoked_owner_device_cannot_publish() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (_, mut entries) = owner_log(&owner, &second);
        let revoke = sign_entry(
            LogEntry {
                root_key: public(&owner),
                sequence: 3,
                previous_hash: entries.last().expect("entries").hash(),
                action: Action::Revoke,
                subject_signing_key: public(&second),
                subject_encryption_key: [0; ENCRYPTION_KEY_BYTES],
                at_unix_ns: NOW + 2,
                author_key: public(&owner),
                signature: [0; SIGNATURE_BYTES],
            },
            &owner,
        );
        entries.push(revoke);
        let log = DeviceLog::replay(&entries).expect("a revocation");
        let store = MemoryChainStore::default();

        let mut published = revision(&owner, 1, NO_PREVIOUS_REVISION);
        published.author_key = public(&second);
        let published = sign_revision(published, &second);

        assert_eq!(
            accept(&published, &log, &store).await,
            Err(ChainError::RevokedAuthor { number: 1 })
        );
    }

    #[tokio::test]
    async fn a_device_outside_the_owners_log_cannot_publish() {
        let (owner, second, stranger) = (key(OWNER_SEED), key(SECOND_SEED), key(STRANGER_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let mut published = revision(&owner, 1, NO_PREVIOUS_REVISION);
        published.author_key = public(&stranger);
        let published = sign_revision(published, &stranger);

        assert_eq!(
            accept(&published, &log, &store).await,
            Err(ChainError::UnknownAuthor { number: 1 })
        );
    }

    #[tokio::test]
    async fn a_revision_verified_against_the_wrong_persons_log_proves_nothing() {
        let (owner, second, stranger) = (key(OWNER_SEED), key(SECOND_SEED), key(STRANGER_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let mut published = revision(&owner, 1, NO_PREVIOUS_REVISION);
        published.owner_root_key = public(&stranger);
        let published = sign_revision(published, &owner);

        assert_eq!(
            accept(&published, &log, &store).await,
            Err(ChainError::NotTheOwner { number: 1 })
        );
    }

    #[tokio::test]
    async fn a_forged_signature_is_refused_before_any_store_read() {
        let (owner, second, stranger) = (key(OWNER_SEED), key(SECOND_SEED), key(STRANGER_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let mut forged = revision(&owner, 1, NO_PREVIOUS_REVISION);
        forged.signature = stranger.sign(&forged.signing_payload()).to_bytes();

        assert_eq!(
            accept(&forged, &log, &store).await,
            Err(ChainError::ForgedSignature { number: 1 })
        );
        assert_eq!(
            store.highest(COLLECTION).await.expect("a healthy store"),
            None,
            "nothing was recorded for a revision that never verified"
        );
    }

    /// Nothing forged. An older genuine revision, offered as the current one
    /// to undo a member removal.
    #[tokio::test]
    async fn an_older_revision_is_refused_as_a_rollback() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);
        accept(&first, &log, &store).await.expect("revision 1");
        let second_revision = revision(&owner, 2, first.hash());
        accept(&second_revision, &log, &store)
            .await
            .expect("revision 2");

        assert_eq!(
            accept(&first, &log, &store).await,
            Err(ChainError::Rollback {
                held: 2,
                offered: 1
            })
        );
    }

    /// Two valid revisions with one number. Both verify; the first seen wins,
    /// and the caller is told rather than the choice being hidden.
    #[tokio::test]
    async fn a_second_revision_at_the_same_number_is_a_fork_and_the_first_is_kept() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let kept = revision(&owner, 1, NO_PREVIOUS_REVISION);
        accept(&kept, &log, &store).await.expect("revision 1");

        // Same number, same owner, genuinely signed, different content.
        let rival = sign_revision(
            Revision {
                manifest_hash: [0x77; 32],
                ..kept.clone()
            },
            &owner,
        );
        assert!(rival.verify(), "the fork is valid on its own");

        assert_eq!(
            verify(&rival, &log, &store, None, &[]).await,
            Err(ChainError::Fork {
                number: 1,
                kept: kept.hash(),
                refused: rival.hash(),
            })
        );
        assert_eq!(
            store
                .highest(COLLECTION)
                .await
                .expect("a healthy store")
                .expect("still held")
                .revision_hash,
            kept.hash(),
            "the first seen is kept, untouched by the second"
        );
    }

    #[tokio::test]
    async fn a_chain_may_not_skip_start_midway_or_relink() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        // Nothing held, and offered a revision from the middle: a reader
        // would never learn what it had missed.
        let midway = revision(&owner, 4, [7; REVISION_HASH_BYTES]);
        assert_eq!(
            accept(&midway, &log, &store).await,
            Err(ChainError::NotTheFirst { number: 4 })
        );

        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);
        accept(&first, &log, &store).await.expect("revision 1");

        let skipped = revision(&owner, 3, first.hash());
        assert_eq!(
            accept(&skipped, &log, &store).await,
            Err(ChainError::SequenceGap {
                expected: 2,
                actual: 3
            })
        );

        // Right number, wrong ancestor: rewriting history needs redoing every
        // revision after it.
        let relinked = revision(&owner, 2, [7; REVISION_HASH_BYTES]);
        assert_eq!(
            accept(&relinked, &log, &store).await,
            Err(ChainError::ChainBroken { number: 2 })
        );
    }

    #[tokio::test]
    async fn a_revision_naming_another_manifest_is_refused() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();
        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);

        assert_eq!(
            verify(&first, &log, &store, Some([0x99; 32]), &[]).await,
            Err(ChainError::ManifestMismatch { number: 1 })
        );

        // Not yet fetched is not a mismatch; the check is then the caller's.
        assert!(verify(&first, &log, &store, None, &[]).await.is_ok());
    }

    #[tokio::test]
    async fn a_malformed_revision_is_refused_before_anything_else() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();

        let unnumbered = Revision {
            number: 0,
            ..revision(&owner, 1, NO_PREVIOUS_REVISION)
        };

        assert!(matches!(
            accept(&unnumbered, &log, &store).await,
            Err(ChainError::Malformed(_))
        ));
    }

    /// A member whose log has moved since the seal. Not a failure — a job.
    #[tokio::test]
    async fn a_member_whose_device_log_moved_is_reported_as_owed_a_reseal() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let store = MemoryChainStore::default();
        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);

        let moved: LogHash = [0x99; 32];
        let accepted = verify(
            &first,
            &log,
            &store,
            Some(MANIFEST),
            &[
                ([2; DEVICE_KEY_BYTES], moved),
                ([3; DEVICE_KEY_BYTES], [0x81; 32]),
                ([9; DEVICE_KEY_BYTES], moved),
            ],
        )
        .await
        .expect("a valid revision");

        assert_eq!(
            accepted.reseal_owed,
            vec![[2; DEVICE_KEY_BYTES]],
            "only the member whose log actually moved, and only if a member"
        );
    }

    #[tokio::test]
    async fn a_store_that_cannot_answer_is_reported_rather_than_guessed_at() {
        let (owner, second) = (key(OWNER_SEED), key(SECOND_SEED));
        let (log, _) = owner_log(&owner, &second);
        let first = revision(&owner, 1, NO_PREVIOUS_REVISION);

        assert_eq!(
            verify(&first, &log, &Broken { fail_read: true }, None, &[]).await,
            Err(ChainError::Store(ChainStoreError::new("the disk is gone")))
        );
        // A write that fails after verification is still not an accepted
        // revision: recording it is what makes the next one checkable.
        assert_eq!(
            verify(&first, &log, &Broken::default(), None, &[]).await,
            Err(ChainError::Store(ChainStoreError::new("the disk is full")))
        );
    }

    #[tokio::test]
    async fn the_memory_store_answers_for_each_collection_separately() {
        let store = MemoryChainStore::default();
        let other = [0x55; SHARE_ID_BYTES];
        let state = ChainState {
            collection_id: COLLECTION,
            number: 3,
            revision_hash: [1; REVISION_HASH_BYTES],
        };

        assert_eq!(store.highest(COLLECTION).await.expect("healthy"), None);
        store.record(state).await.expect("records");

        assert_eq!(
            store.highest(COLLECTION).await.expect("healthy"),
            Some(state)
        );
        assert_eq!(store.highest(other).await.expect("healthy"), None);
    }

    #[test]
    fn a_store_failure_says_what_could_not_be_reached() {
        let error = ChainStoreError::new("the disk is gone");

        assert_eq!(
            error.to_string(),
            "the chain store is unavailable: the disk is gone"
        );
        assert_eq!(
            ChainError::from(error).to_string(),
            "the chain store is unavailable: the disk is gone"
        );
    }
}
