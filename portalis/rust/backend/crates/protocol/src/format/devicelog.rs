//! A person's devices, as a signed append-only log.
//!
//! This is `SPEC.md` §7.2, and decision D2: a person is a device log, not an
//! account row. The distinction matters for one attack. If the service holds
//! the list, a hostile service can add a device to it, and an owner about to
//! seal a content key will seal to that device — the theft is silent and
//! nothing downstream can notice. A log makes it impossible rather than
//! merely auditable: every entry is signed by a device the log already
//! enrols, so extending it needs a key that is already inside.
//!
//! ```text
//! entry := "portalis.devicelog.v1\0"
//!          u8[32]  root_key                first device's Ed25519 key
//!          u64     sequence                1 at the root, then +1
//!          u8[32]  previous_hash           zero at the root
//!          u8      action                  1 = enrol, 2 = revoke
//!          u8[32]  subject_signing_key
//!          u8[32]  subject_encryption_key  zero for a revocation
//!          u64     at_unix_ns
//!          u8[32]  author_key              an enrolled, unrevoked device
//!          u8[64]  signature               over every preceding field
//! ```
//!
//! Replay is the whole interface: hand it entries and it returns the device
//! set they authorize, or the first reason they do not. Every rule is checked
//! at the point the entry claims to occupy, not against the final state —
//! a device revoked at sequence 5 cannot author at sequence 6, but the entries
//! it authored at sequence 3 stand.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, SIGNATURE_BYTES};

/// Mixed into every signing payload, so a device log signature cannot be
/// lifted onto anything else this protocol signs.
const DOMAIN: &[u8] = b"portalis.devicelog.v1\0";

/// The hash that chains one entry to the next, and names a log's state.
pub const LOG_HASH_BYTES: usize = 32;
pub type LogHash = [u8; LOG_HASH_BYTES];

/// The root entry's `previous_hash`: it chains to nothing.
pub const NO_PREVIOUS: LogHash = [0; LOG_HASH_BYTES];

/// What an entry does to its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Enrol,
    Revoke,
}

impl Action {
    const ENROL: u8 = 1;
    const REVOKE: u8 = 2;

    const fn code(self) -> u8 {
        match self {
            Self::Enrol => Self::ENROL,
            Self::Revoke => Self::REVOKE,
        }
    }
}

/// One signed statement about one device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// The first device's key, repeated in every entry so an entry cannot be
    /// lifted from one person's log into another's.
    pub root_key: [u8; DEVICE_KEY_BYTES],
    pub sequence: u64,
    pub previous_hash: LogHash,
    pub action: Action,
    pub subject_signing_key: [u8; DEVICE_KEY_BYTES],
    /// Zero for a revocation, which grants nothing and so needs no key.
    pub subject_encryption_key: [u8; ENCRYPTION_KEY_BYTES],
    pub at_unix_ns: u64,
    pub author_key: [u8; DEVICE_KEY_BYTES],
    pub signature: [u8; SIGNATURE_BYTES],
}

impl LogEntry {
    /// Every field before the signature, in order.
    ///
    /// A caller signs this and puts the result in [`Self::signature`]. It is
    /// public because the signing device is not always the verifying one.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DOMAIN.len() + 185);
        bytes.extend_from_slice(DOMAIN);
        bytes.extend_from_slice(&self.root_key);
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&self.previous_hash);
        bytes.push(self.action.code());
        bytes.extend_from_slice(&self.subject_signing_key);
        bytes.extend_from_slice(&self.subject_encryption_key);
        bytes.extend_from_slice(&self.at_unix_ns.to_le_bytes());
        bytes.extend_from_slice(&self.author_key);
        bytes
    }

    /// This entry's name, and the `previous_hash` the next one must carry.
    ///
    /// Over the signature as well as the payload, so two entries that differ
    /// only in who signed them are different entries.
    #[must_use]
    pub fn hash(&self) -> LogHash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.signing_payload());
        hasher.update(&self.signature);
        *hasher.finalize().as_bytes()
    }

    /// Whether the author's key actually signed this entry.
    ///
    /// Says nothing about whether that author was allowed to: that is
    /// [`DeviceLog::replay`]'s job, and it depends on where the entry sits.
    #[must_use]
    pub fn verify(&self) -> bool {
        let Ok(author) = VerifyingKey::from_bytes(&self.author_key) else {
            return false;
        };
        author
            .verify_strict(
                &self.signing_payload(),
                &Signature::from_bytes(&self.signature),
            )
            .is_ok()
    }
}

/// A device the log authorizes, or once did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Device {
    pub signing_key: [u8; DEVICE_KEY_BYTES],
    pub encryption_key: [u8; ENCRYPTION_KEY_BYTES],
    pub enrolled_at_unix_ns: u64,
    /// `None` while the device is still authorized.
    pub revoked_at_unix_ns: Option<u64>,
}

impl Device {
    #[must_use]
    pub const fn is_authorized(&self) -> bool {
        self.revoked_at_unix_ns.is_none()
    }
}

/// Why a log does not authorize what it claims to.
///
/// Each variant names one rule, because "invalid log" tells a user nothing
/// and tells an adversarial test even less.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DeviceLogError {
    #[error("a device log has no entries")]
    Empty,
    #[error("the first entry is at sequence {actual}, and a log begins at 1")]
    NotRooted { actual: u64 },
    #[error("the root entry at sequence 1 is not signed by the root device")]
    RootNotSelfSigned,
    #[error("the entry at sequence {sequence} is a second root")]
    SecondRoot { sequence: u64 },
    #[error("sequence {actual} follows {expected}, so entries are missing or reordered")]
    SequenceGap { expected: u64, actual: u64 },
    #[error(
        "the entry at sequence {sequence} names a previous entry that is not the one before it"
    )]
    ChainBroken { sequence: u64 },
    #[error("the entry at sequence {sequence} belongs to another person's log")]
    WrongRoot { sequence: u64 },
    #[error("the entry at sequence {sequence} is signed by a device this log never enrolled")]
    UnknownAuthor { sequence: u64 },
    #[error("the entry at sequence {sequence} is signed by a device revoked before it")]
    RevokedAuthor { sequence: u64 },
    #[error("the signature on the entry at sequence {sequence} is not the author's")]
    ForgedSignature { sequence: u64 },
    #[error("the entry at sequence {sequence} enrols a device the log already enrolled")]
    AlreadyEnrolled { sequence: u64 },
    #[error("the entry at sequence {sequence} revokes a device the log never enrolled")]
    UnknownSubject { sequence: u64 },
    #[error(
        "the revocation at sequence {sequence} carries an encryption key, which grants nothing and must be zero"
    )]
    RevocationCarriesKey { sequence: u64 },
    /// A log offered as a replacement that is behind the one already held.
    /// The dangerous version of this is a service serving an old log to undo
    /// a revocation the owner has already published.
    #[error("the offered log ends at sequence {offered}, behind the {held} already verified")]
    StaleLog { held: u64, offered: u64 },
    /// Same root, same length or longer, but it disagrees about history.
    #[error("the offered log disagrees with the held one at sequence {sequence}")]
    ForkedLog { sequence: u64 },
}

/// The verified state a log replays to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceLog {
    root_key: [u8; DEVICE_KEY_BYTES],
    devices: BTreeMap<[u8; DEVICE_KEY_BYTES], Device>,
    sequence: u64,
    head: LogHash,
}

impl DeviceLog {
    /// Replays entries into the device set they authorize.
    ///
    /// # Errors
    ///
    /// Returns the first [`DeviceLogError`] the entries break, so a caller
    /// and an adversarial test see the same reason.
    pub fn replay(entries: &[LogEntry]) -> Result<Self, DeviceLogError> {
        Self::replay_checked(entries, None)
    }

    /// Accepts a log offered as a replacement for this one.
    ///
    /// The offered log must be the same person's, must reach at least as far,
    /// and must agree with this one everywhere this one has already seen. A
    /// service that serves an old log — undoing a revocation — or a different
    /// one — splitting what two contacts believe — is refused rather than
    /// silently adopted.
    ///
    /// # Errors
    ///
    /// Returns [`DeviceLogError`] when the offered entries are invalid on
    /// their own, are behind this log, or disagree with it.
    pub fn adopt(&self, entries: &[LogEntry]) -> Result<Self, DeviceLogError> {
        let offered = Self::replay_checked(entries, Some((self.sequence, self.head)))?;
        if offered.root_key != self.root_key {
            return Err(DeviceLogError::WrongRoot { sequence: 1 });
        }
        if offered.sequence < self.sequence {
            return Err(DeviceLogError::StaleLog {
                held: self.sequence,
                offered: offered.sequence,
            });
        }
        Ok(offered)
    }

    /// The devices that may author entries, receive sealed keys, and sign
    /// manifest entries, in a stable order.
    #[must_use]
    pub fn authorized(&self) -> Vec<Device> {
        self.devices
            .values()
            .filter(|device| device.is_authorized())
            .copied()
            .collect()
    }

    /// Every device the log has ever named, authorized or not.
    #[must_use]
    pub fn history(&self) -> Vec<Device> {
        self.devices.values().copied().collect()
    }

    /// Whether this device may act for its owner right now.
    #[must_use]
    pub fn is_authorized(&self, signing_key: &[u8; DEVICE_KEY_BYTES]) -> bool {
        self.devices
            .get(signing_key)
            .is_some_and(Device::is_authorized)
    }

    /// The person this log belongs to.
    #[must_use]
    pub const fn root_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.root_key
    }

    /// How far the log reaches.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// The log's state, as recorded in a revision's `device_log_hash` so a
    /// contact can tell that a re-seal is owed rather than wondering why a
    /// newly linked device opens nothing.
    #[must_use]
    pub const fn hash(&self) -> LogHash {
        self.head
    }

    /// Replays entries, optionally checking that the running hash at a
    /// sequence already verified is the one already held.
    fn replay_checked(
        entries: &[LogEntry],
        checkpoint: Option<(u64, LogHash)>,
    ) -> Result<Self, DeviceLogError> {
        let (root, rest) = entries.split_first().ok_or(DeviceLogError::Empty)?;
        let mut log = Self::root(root)?;

        for entry in rest {
            log.apply(entry)?;
        }

        if let Some((sequence, held)) = checkpoint {
            // Only meaningful once the offered log is long enough to have
            // reached it; a shorter one is stale, which `adopt` reports.
            if log.sequence >= sequence {
                let at_checkpoint = entries
                    .get(usize::try_from(sequence - 1).unwrap_or(usize::MAX))
                    .map(LogEntry::hash);
                if at_checkpoint != Some(held) {
                    return Err(DeviceLogError::ForkedLog { sequence });
                }
            }
        }

        Ok(log)
    }

    /// Opens a log at its root, which is the only self-signed entry.
    fn root(entry: &LogEntry) -> Result<Self, DeviceLogError> {
        if entry.sequence != 1 {
            return Err(DeviceLogError::NotRooted {
                actual: entry.sequence,
            });
        }
        if entry.previous_hash != NO_PREVIOUS {
            return Err(DeviceLogError::ChainBroken { sequence: 1 });
        }
        // The root device enrols itself, and nothing else can: there is no
        // earlier entry to have authorized another author.
        if entry.author_key != entry.root_key
            || entry.subject_signing_key != entry.root_key
            || entry.action != Action::Enrol
        {
            return Err(DeviceLogError::RootNotSelfSigned);
        }
        if !entry.verify() {
            return Err(DeviceLogError::ForgedSignature { sequence: 1 });
        }

        let mut devices = BTreeMap::new();
        devices.insert(
            entry.root_key,
            Device {
                signing_key: entry.root_key,
                encryption_key: entry.subject_encryption_key,
                enrolled_at_unix_ns: entry.at_unix_ns,
                revoked_at_unix_ns: None,
            },
        );
        Ok(Self {
            root_key: entry.root_key,
            devices,
            sequence: 1,
            head: entry.hash(),
        })
    }

    /// Applies one entry to the state the entries before it produced.
    fn apply(&mut self, entry: &LogEntry) -> Result<(), DeviceLogError> {
        let sequence = entry.sequence;
        if sequence == 1 {
            return Err(DeviceLogError::SecondRoot { sequence });
        }
        if sequence != self.sequence + 1 {
            return Err(DeviceLogError::SequenceGap {
                expected: self.sequence + 1,
                actual: sequence,
            });
        }
        if entry.root_key != self.root_key {
            return Err(DeviceLogError::WrongRoot { sequence });
        }
        if entry.previous_hash != self.head {
            return Err(DeviceLogError::ChainBroken { sequence });
        }

        // Authority is checked where the entry sits, so a device revoked
        // later still authored validly earlier.
        match self.devices.get(&entry.author_key) {
            None => return Err(DeviceLogError::UnknownAuthor { sequence }),
            Some(author) if !author.is_authorized() => {
                return Err(DeviceLogError::RevokedAuthor { sequence });
            }
            Some(_) => {}
        }
        if !entry.verify() {
            return Err(DeviceLogError::ForgedSignature { sequence });
        }

        match entry.action {
            Action::Enrol => {
                if self.devices.contains_key(&entry.subject_signing_key) {
                    return Err(DeviceLogError::AlreadyEnrolled { sequence });
                }
                self.devices.insert(
                    entry.subject_signing_key,
                    Device {
                        signing_key: entry.subject_signing_key,
                        encryption_key: entry.subject_encryption_key,
                        enrolled_at_unix_ns: entry.at_unix_ns,
                        revoked_at_unix_ns: None,
                    },
                );
            }
            Action::Revoke => {
                if entry.subject_encryption_key != [0; ENCRYPTION_KEY_BYTES] {
                    return Err(DeviceLogError::RevocationCarriesKey { sequence });
                }
                let subject = self
                    .devices
                    .get_mut(&entry.subject_signing_key)
                    .ok_or(DeviceLogError::UnknownSubject { sequence })?;
                // Revoking twice is the same statement, and the first one is
                // when authority ended.
                subject.revoked_at_unix_ns.get_or_insert(entry.at_unix_ns);
            }
        }

        self.sequence = sequence;
        self.head = entry.hash();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! A log is only worth having if it refuses things, so most of what
    //! follows is refusals. Each one names the rule it breaks, because
    //! "invalid log" would let two different bugs pass the same assertion.

    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    const ROOT_SEED: [u8; 32] = [1; 32];
    const SECOND_SEED: [u8; 32] = [2; 32];
    const THIRD_SEED: [u8; 32] = [3; 32];
    const STRANGER_SEED: [u8; 32] = [9; 32];
    const NOW: u64 = 1_700_000_000_000_000_000;

    fn key(seed: [u8; 32]) -> SigningKey {
        SigningKey::from_bytes(&seed)
    }

    fn public(signer: &SigningKey) -> [u8; DEVICE_KEY_BYTES] {
        signer.verifying_key().to_bytes()
    }

    /// The encryption key a device publishes alongside its signing key. Its
    /// value is arbitrary here; only that it survives replay matters.
    fn encryption_key(seed: [u8; 32]) -> [u8; ENCRYPTION_KEY_BYTES] {
        [seed[0].wrapping_add(0x40); ENCRYPTION_KEY_BYTES]
    }

    /// Signs whatever it is handed, so a test can build an entry that breaks
    /// exactly one rule and remains genuinely signed.
    fn sign(mut entry: LogEntry, author: &SigningKey) -> LogEntry {
        entry.signature = author.sign(&entry.signing_payload()).to_bytes();
        entry
    }

    fn root_entry(root: &SigningKey) -> LogEntry {
        sign(
            LogEntry {
                root_key: public(root),
                sequence: 1,
                previous_hash: NO_PREVIOUS,
                action: Action::Enrol,
                subject_signing_key: public(root),
                subject_encryption_key: encryption_key(ROOT_SEED),
                at_unix_ns: NOW,
                author_key: public(root),
                signature: [0; SIGNATURE_BYTES],
            },
            root,
        )
    }

    /// The next entry in a log, signed by `author`.
    fn next(
        previous: &LogEntry,
        action: Action,
        subject: &SigningKey,
        subject_seed: [u8; 32],
        author: &SigningKey,
    ) -> LogEntry {
        sign(
            LogEntry {
                root_key: previous.root_key,
                sequence: previous.sequence + 1,
                previous_hash: previous.hash(),
                action,
                subject_signing_key: public(subject),
                subject_encryption_key: match action {
                    Action::Enrol => encryption_key(subject_seed),
                    Action::Revoke => [0; ENCRYPTION_KEY_BYTES],
                },
                at_unix_ns: previous.at_unix_ns + 1,
                author_key: public(author),
                signature: [0; SIGNATURE_BYTES],
            },
            author,
        )
    }

    /// Root enrols itself, then a second device, then a third.
    fn three_devices() -> (SigningKey, Vec<LogEntry>) {
        let (root, second, third) = (key(ROOT_SEED), key(SECOND_SEED), key(THIRD_SEED));
        let first = root_entry(&root);
        let enrol_second = next(&first, Action::Enrol, &second, SECOND_SEED, &root);
        let enrol_third = next(&enrol_second, Action::Enrol, &third, THIRD_SEED, &root);
        (root, vec![first, enrol_second, enrol_third])
    }

    #[test]
    fn a_root_entry_alone_authorizes_the_device_that_signed_it() {
        let root = key(ROOT_SEED);
        let entries = vec![root_entry(&root)];

        let log = DeviceLog::replay(&entries).expect("a rooted log");

        assert_eq!(log.root_key(), public(&root));
        assert_eq!(log.sequence(), 1);
        assert_eq!(log.hash(), entries[0].hash());
        assert!(log.is_authorized(&public(&root)));
        assert_eq!(
            log.authorized(),
            vec![Device {
                signing_key: public(&root),
                encryption_key: encryption_key(ROOT_SEED),
                enrolled_at_unix_ns: NOW,
                revoked_at_unix_ns: None,
            }]
        );
    }

    #[test]
    fn enrolling_and_revoking_moves_a_device_in_and_out_of_authority() {
        let (root, mut entries) = three_devices();
        let third = key(THIRD_SEED);

        let log = DeviceLog::replay(&entries).expect("three devices");
        assert_eq!(log.authorized().len(), 3);
        assert_eq!(log.sequence(), 3);

        let revoke = next(
            entries.last().expect("entries"),
            Action::Revoke,
            &third,
            THIRD_SEED,
            &root,
        );
        entries.push(revoke);
        let log = DeviceLog::replay(&entries).expect("a revocation");

        assert_eq!(log.authorized().len(), 2);
        assert!(!log.is_authorized(&public(&third)));
        // Revoking ends authority; it does not erase that the device existed,
        // which is what a later audit and a key rotation both need.
        assert_eq!(log.history().len(), 3);
        assert_eq!(
            log.history()
                .into_iter()
                .find(|device| device.signing_key == public(&third))
                .and_then(|device| device.revoked_at_unix_ns),
            Some(NOW + 3)
        );
    }

    /// The rule the whole design rests on: a device already inside the log
    /// must have authorized every extension.
    #[test]
    fn a_device_the_log_never_enrolled_cannot_extend_it() {
        let (_, entries) = three_devices();
        let stranger = key(STRANGER_SEED);
        let injected = next(
            entries.last().expect("entries"),
            Action::Enrol,
            &stranger,
            STRANGER_SEED,
            &stranger,
        );

        let mut attacked = entries;
        attacked.push(injected);

        assert_eq!(
            DeviceLog::replay(&attacked),
            Err(DeviceLogError::UnknownAuthor { sequence: 4 })
        );
    }

    #[test]
    fn a_device_revoked_earlier_cannot_author_later() {
        let (root, mut entries) = three_devices();
        let (second, third) = (key(SECOND_SEED), key(THIRD_SEED));

        let revoke_second = next(
            entries.last().expect("entries"),
            Action::Revoke,
            &second,
            SECOND_SEED,
            &root,
        );
        entries.push(revoke_second);
        // Genuinely signed by the revoked device, and refused for who signed
        // it rather than for the signature itself.
        let after = next(
            entries.last().expect("entries"),
            Action::Revoke,
            &third,
            THIRD_SEED,
            &second,
        );
        entries.push(after);

        assert_eq!(
            DeviceLog::replay(&entries),
            Err(DeviceLogError::RevokedAuthor { sequence: 5 })
        );
    }

    #[test]
    fn a_forged_signature_is_refused_at_the_root_and_after_it() {
        let (root, mut entries) = three_devices();
        let stranger = key(STRANGER_SEED);

        let mut forged_root = root_entry(&root);
        forged_root.signature = stranger.sign(b"something else").to_bytes();
        assert_eq!(
            DeviceLog::replay(&[forged_root]),
            Err(DeviceLogError::ForgedSignature { sequence: 1 })
        );

        // Claims the root as author, signed by someone else.
        let mut forged = next(
            entries.last().expect("entries"),
            Action::Enrol,
            &stranger,
            STRANGER_SEED,
            &root,
        );
        forged.signature = stranger.sign(&forged.signing_payload()).to_bytes();
        entries.push(forged);
        assert_eq!(
            DeviceLog::replay(&entries),
            Err(DeviceLogError::ForgedSignature { sequence: 4 })
        );
    }

    #[test]
    fn an_author_key_that_is_not_a_curve_point_verifies_nothing() {
        let root = key(ROOT_SEED);
        let off_curve = (2_u8..=u8::MAX)
            .map(|byte| [byte; DEVICE_KEY_BYTES])
            .find(|bytes| VerifyingKey::from_bytes(bytes).is_err())
            .expect("some byte pattern is not a curve point");
        let mut entry = root_entry(&root);
        entry.root_key = off_curve;
        entry.subject_signing_key = off_curve;
        entry.author_key = off_curve;

        assert!(!entry.verify());
        assert_eq!(
            DeviceLog::replay(&[entry]),
            Err(DeviceLogError::ForgedSignature { sequence: 1 })
        );
    }

    #[test]
    fn a_log_must_begin_at_a_self_signed_root() {
        let (root, entries) = three_devices();
        let second = key(SECOND_SEED);

        assert_eq!(DeviceLog::replay(&[]), Err(DeviceLogError::Empty));
        assert_eq!(
            DeviceLog::replay(&entries[1..]),
            Err(DeviceLogError::NotRooted { actual: 2 })
        );

        // Sequence 1, but enrolling someone other than the signer: the root
        // is the one entry that cannot speak about another device.
        let mut not_self = root_entry(&root);
        not_self.subject_signing_key = public(&second);
        let not_self = sign(not_self, &root);
        assert_eq!(
            DeviceLog::replay(&[not_self]),
            Err(DeviceLogError::RootNotSelfSigned)
        );

        // Signed by the root device but claiming another root key.
        let mut wrong_root = root_entry(&root);
        wrong_root.root_key = public(&second);
        let wrong_root = sign(wrong_root, &root);
        assert_eq!(
            DeviceLog::replay(&[wrong_root]),
            Err(DeviceLogError::RootNotSelfSigned)
        );

        // A root that revokes rather than enrols authorizes nobody.
        let mut revoking_root = root_entry(&root);
        revoking_root.action = Action::Revoke;
        let revoking_root = sign(revoking_root, &root);
        assert_eq!(
            DeviceLog::replay(&[revoking_root]),
            Err(DeviceLogError::RootNotSelfSigned)
        );

        // A root chained to something cannot be a beginning.
        let mut chained_root = root_entry(&root);
        chained_root.previous_hash = [7; LOG_HASH_BYTES];
        let chained_root = sign(chained_root, &root);
        assert_eq!(
            DeviceLog::replay(&[chained_root]),
            Err(DeviceLogError::ChainBroken { sequence: 1 })
        );
    }

    #[test]
    fn a_second_root_cannot_be_appended() {
        let (root, mut entries) = three_devices();
        entries.push(root_entry(&root));

        assert_eq!(
            DeviceLog::replay(&entries),
            Err(DeviceLogError::SecondRoot { sequence: 1 })
        );
    }

    #[test]
    fn reordered_truncated_and_relinked_entries_are_refused() {
        let (_, entries) = three_devices();

        let mut reordered = entries.clone();
        reordered.swap(1, 2);
        assert_eq!(
            DeviceLog::replay(&reordered),
            Err(DeviceLogError::SequenceGap {
                expected: 2,
                actual: 3
            })
        );

        // Dropping the middle entry leaves a gap the sequence exposes.
        let gapped = vec![entries[0], entries[2]];
        assert_eq!(
            DeviceLog::replay(&gapped),
            Err(DeviceLogError::SequenceGap {
                expected: 2,
                actual: 3
            })
        );

        // Right sequence, wrong ancestor: the hash chain is what makes
        // rewriting an earlier entry impossible without redoing every one
        // after it.
        let (root, second) = (key(ROOT_SEED), key(SECOND_SEED));
        let mut relinked = entries.clone();
        let mut tampered = relinked[1];
        tampered.previous_hash = [7; LOG_HASH_BYTES];
        relinked[1] = sign(tampered, &root);
        assert_eq!(
            DeviceLog::replay(&relinked),
            Err(DeviceLogError::ChainBroken { sequence: 2 })
        );

        // An entry lifted from another person's log, resigned to fit.
        let mut lifted = entries.clone();
        let mut foreign = lifted[1];
        foreign.root_key = public(&second);
        lifted[1] = sign(foreign, &root);
        assert_eq!(
            DeviceLog::replay(&lifted),
            Err(DeviceLogError::WrongRoot { sequence: 2 })
        );
    }

    #[test]
    fn a_subject_is_enrolled_once_and_revoked_only_if_known() {
        let (root, entries) = three_devices();
        let (second, stranger) = (key(SECOND_SEED), key(STRANGER_SEED));

        let mut twice = entries.clone();
        twice.push(next(
            entries.last().expect("entries"),
            Action::Enrol,
            &second,
            SECOND_SEED,
            &root,
        ));
        assert_eq!(
            DeviceLog::replay(&twice),
            Err(DeviceLogError::AlreadyEnrolled { sequence: 4 })
        );

        let mut unknown = entries.clone();
        unknown.push(next(
            entries.last().expect("entries"),
            Action::Revoke,
            &stranger,
            STRANGER_SEED,
            &root,
        ));
        assert_eq!(
            DeviceLog::replay(&unknown),
            Err(DeviceLogError::UnknownSubject { sequence: 4 })
        );

        // A revocation grants nothing, so carrying a key is a sign the entry
        // was built by something that does not understand the format.
        let mut carries = entries.clone();
        let mut revoke = next(
            entries.last().expect("entries"),
            Action::Revoke,
            &second,
            SECOND_SEED,
            &root,
        );
        revoke.subject_encryption_key = encryption_key(SECOND_SEED);
        carries.push(sign(revoke, &root));
        assert_eq!(
            DeviceLog::replay(&carries),
            Err(DeviceLogError::RevocationCarriesKey { sequence: 4 })
        );
    }

    /// Revoking twice says the same thing, and the first one is when
    /// authority actually ended.
    #[test]
    fn revoking_the_same_device_twice_keeps_the_first_time() {
        let (root, mut entries) = three_devices();
        let third = key(THIRD_SEED);

        for _ in 0..2 {
            let revoke = next(
                entries.last().expect("entries"),
                Action::Revoke,
                &third,
                THIRD_SEED,
                &root,
            );
            entries.push(revoke);
        }

        let log = DeviceLog::replay(&entries).expect("an idempotent revocation");

        assert_eq!(log.sequence(), 5);
        assert_eq!(
            log.history()
                .into_iter()
                .find(|device| device.signing_key == public(&third))
                .and_then(|device| device.revoked_at_unix_ns),
            Some(NOW + 3),
            "authority ended at the first revocation, not the second"
        );
    }

    #[test]
    fn a_log_is_adopted_only_when_it_extends_the_one_already_held() {
        let (root, entries) = three_devices();
        let third = key(THIRD_SEED);
        let held = DeviceLog::replay(&entries).expect("three devices");

        // Longer, same history: adopted.
        let mut extended = entries.clone();
        extended.push(next(
            entries.last().expect("entries"),
            Action::Revoke,
            &third,
            THIRD_SEED,
            &root,
        ));
        let adopted = held.adopt(&extended).expect("an extension");
        assert_eq!(adopted.sequence(), 4);
        assert!(!adopted.is_authorized(&public(&third)));

        // The same log again is not stale — it is where we already are.
        assert_eq!(held.adopt(&entries).expect("the same log"), held);

        // Shorter: the attack that undoes a revocation by serving an old log.
        let stale = &entries[..2];
        assert_eq!(
            adopted.adopt(stale),
            Err(DeviceLogError::StaleLog {
                held: 4,
                offered: 2
            })
        );
    }

    #[test]
    fn a_log_that_disagrees_about_history_is_a_fork_not_an_update() {
        let (root, entries) = three_devices();
        let (second, stranger) = (key(SECOND_SEED), key(STRANGER_SEED));
        let held = DeviceLog::replay(&entries).expect("three devices");

        // Same root, same length, different third entry: internally valid,
        // and irreconcilable with what we already verified.
        let forked_third = next(&entries[1], Action::Enrol, &stranger, STRANGER_SEED, &root);
        let forked = vec![entries[0], entries[1], forked_third];
        assert!(
            DeviceLog::replay(&forked).is_ok(),
            "the fork is valid on its own, which is what makes it dangerous"
        );
        assert_eq!(
            held.adopt(&forked),
            Err(DeviceLogError::ForkedLog { sequence: 3 })
        );

        // A different person's log entirely, valid in itself.
        let other_root = root_entry(&second);
        let other = vec![
            other_root,
            next(
                &other_root,
                Action::Enrol,
                &stranger,
                STRANGER_SEED,
                &second,
            ),
            next(
                &next(
                    &other_root,
                    Action::Enrol,
                    &stranger,
                    STRANGER_SEED,
                    &second,
                ),
                Action::Revoke,
                &stranger,
                STRANGER_SEED,
                &second,
            ),
        ];
        assert_eq!(
            held.adopt(&other),
            Err(DeviceLogError::ForkedLog { sequence: 3 })
        );
    }

    /// The signing payload is what two implementations must agree on, so its
    /// shape is pinned rather than left to whatever the struct happens to do.
    #[test]
    fn the_signing_payload_covers_every_field_before_the_signature() {
        let root = key(ROOT_SEED);
        let entry = root_entry(&root);
        let payload = entry.signing_payload();

        assert!(payload.starts_with(DOMAIN));
        assert_eq!(
            payload.len(),
            DOMAIN.len() + 32 + 8 + 32 + 1 + 32 + 32 + 8 + 32
        );

        // Every field changes it, and the hash covers the signature too.
        for mutate in [
            (|e: &mut LogEntry| e.sequence += 1) as fn(&mut LogEntry),
            |e| e.previous_hash = [7; LOG_HASH_BYTES],
            |e| e.action = Action::Revoke,
            |e| e.subject_signing_key = [7; DEVICE_KEY_BYTES],
            |e| e.subject_encryption_key = [7; ENCRYPTION_KEY_BYTES],
            |e| e.at_unix_ns += 1,
            |e| e.author_key = [7; DEVICE_KEY_BYTES],
            |e| e.root_key = [7; DEVICE_KEY_BYTES],
        ] {
            let mut changed = entry;
            mutate(&mut changed);
            assert_ne!(changed.signing_payload(), payload);
            assert_ne!(changed.hash(), entry.hash());
        }

        let mut resigned = entry;
        resigned.signature = [7; SIGNATURE_BYTES];
        assert_eq!(resigned.signing_payload(), payload);
        assert_ne!(
            resigned.hash(),
            entry.hash(),
            "the hash names the signature"
        );
    }

    #[test]
    fn an_action_is_one_byte_on_the_wire() {
        assert_eq!(Action::Enrol.code(), 1);
        assert_eq!(Action::Revoke.code(), 2);
        assert!(Action::Enrol < Action::Revoke);
    }
}
