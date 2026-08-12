//! A collection's history, as a chain of signed revisions.
//!
//! This is `SPEC.md` §7.3, and decision D3: a collection is a chain, not a row
//! the service keeps for you. Revision *n* names the hash of revision *n − 1*,
//! so a reader can tell whether what it was handed follows what it already
//! verified — and the service's compare-and-set becomes an optimisation rather
//! than the thing correctness rests on.
//!
//! ```text
//! revision := "portalis.revision.v1\0"
//!             u8[16]  collection_id
//!             u64     number                1 upward, no gaps
//!             u8[32]  previous_hash         zero at revision 1
//!             u8[32]  manifest_hash
//!             u8[32]  owner_root_key
//!             u64     at_unix_ns
//!             u32     member_count
//!             member*                       ascending by root key
//!             u8[32]  author_key            an owner device
//!             u8[64]  signature
//!
//! member   := u8[32]  root_key
//!             u8[32]  device_log_hash       log state the key was sealed against
//! ```
//!
//! This module answers only what a revision says about itself: is it
//! well-formed, and did the key it names sign it. Whether that key was allowed
//! to, and whether this revision belongs after the one already held, need a
//! device log and a held state — which is `client`'s job, because the service
//! must never be the thing that decides.
//!
//! `device_log_hash` is per member and deliberately not part of validity. It
//! records the log state the owner sealed a content key against, so a contact
//! who has since linked a device sees at once that a re-seal is owed instead of
//! wondering why the new device opens nothing.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::format::devicelog::{LOG_HASH_BYTES, LogHash};
use crate::format::manifest::ManifestHash;
use crate::{DEVICE_KEY_BYTES, SHARE_ID_BYTES, SIGNATURE_BYTES};

/// Mixed into every signing payload, so a revision signature cannot be lifted
/// onto anything else this protocol signs.
const DOMAIN: &[u8] = b"portalis.revision.v1\0";

/// A revision's name, and the `previous_hash` the next one must carry.
pub const REVISION_HASH_BYTES: usize = 32;
pub type RevisionHash = [u8; REVISION_HASH_BYTES];

/// The first revision's `previous_hash`: it follows nothing.
pub const NO_PREVIOUS: RevisionHash = [0; REVISION_HASH_BYTES];

/// A ceiling on members, so a decoder cannot be told to allocate without
/// bound before a signature has been checked.
pub const MAX_MEMBERS: usize = 1024;

/// Someone the revision's content key was sealed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Member {
    pub root_key: [u8; DEVICE_KEY_BYTES],
    /// The member's device log state at the moment of sealing. Not part of
    /// validity: a mismatch means a re-seal is owed, not that anything lied.
    pub device_log_hash: LogHash,
}

/// One signed statement about what a collection contains and who may read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub collection_id: [u8; SHARE_ID_BYTES],
    pub number: u64,
    pub previous_hash: RevisionHash,
    pub manifest_hash: ManifestHash,
    /// The owner's device log root, which is the collection's identity across
    /// every device the owner has or will link.
    pub owner_root_key: [u8; DEVICE_KEY_BYTES],
    pub at_unix_ns: u64,
    pub members: Vec<Member>,
    /// The owner device that signed this. Whether it was authorized needs the
    /// owner's device log, which this crate does not have.
    pub author_key: [u8; DEVICE_KEY_BYTES],
    pub signature: [u8; SIGNATURE_BYTES],
}

/// Why a revision is not a revision.
///
/// Only shape and self-consistency: everything requiring outside knowledge is
/// `client`'s to report.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RevisionError {
    #[error("revision {number} is numbered 0, and a chain begins at 1")]
    NotNumbered { number: u64 },
    #[error("revision 1 follows nothing, so its previous hash must be zero")]
    FirstFollowsSomething,
    #[error("revision {number} follows something, so its previous hash must not be zero")]
    FollowsNothing { number: u64 },
    #[error("{actual} members exceeds the {MAX_MEMBERS} limit")]
    TooManyMembers { actual: usize },
    #[error("members must ascend by root key, and member {index} does not")]
    MembersOutOfOrder { index: usize },
    #[error("member {index} appears more than once")]
    DuplicateMember { index: usize },
    #[error("the encoded revision is truncated or holds trailing bytes")]
    Malformed,
    #[error("the encoded revision declares a domain this reader does not speak")]
    UnknownDomain,
}

impl Revision {
    /// Checks everything a revision can be judged on alone.
    ///
    /// Deliberately not the signature: a caller that only needs to know
    /// whether bytes decode into a sane shape should not pay for a curve
    /// operation, and a caller that needs both calls both.
    ///
    /// # Errors
    ///
    /// Returns [`RevisionError`] for a number of zero, a first revision that
    /// claims a predecessor or a later one that claims none, and members that
    /// are too many, out of order, or repeated.
    pub fn validate(&self) -> Result<(), RevisionError> {
        if self.number == 0 {
            return Err(RevisionError::NotNumbered {
                number: self.number,
            });
        }
        if self.number == 1 && self.previous_hash != NO_PREVIOUS {
            return Err(RevisionError::FirstFollowsSomething);
        }
        if self.number > 1 && self.previous_hash == NO_PREVIOUS {
            return Err(RevisionError::FollowsNothing {
                number: self.number,
            });
        }
        if self.members.len() > MAX_MEMBERS {
            return Err(RevisionError::TooManyMembers {
                actual: self.members.len(),
            });
        }
        // Ascending and unique, so one member list has one encoding and a
        // signature over it means the same thing to every reader.
        for (index, pair) in self.members.windows(2).enumerate() {
            match pair[0].root_key.cmp(&pair[1].root_key) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(RevisionError::DuplicateMember { index: index + 1 });
                }
                std::cmp::Ordering::Greater => {
                    return Err(RevisionError::MembersOutOfOrder { index: index + 1 });
                }
            }
        }
        Ok(())
    }

    /// Every field before the signature, in order.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DOMAIN.len() + 156 + self.members.len() * 64);
        bytes.extend_from_slice(DOMAIN);
        bytes.extend_from_slice(&self.collection_id);
        bytes.extend_from_slice(&self.number.to_le_bytes());
        bytes.extend_from_slice(&self.previous_hash);
        bytes.extend_from_slice(&self.manifest_hash);
        bytes.extend_from_slice(&self.owner_root_key);
        bytes.extend_from_slice(&self.at_unix_ns.to_le_bytes());
        // Narrowing cannot lose information: `validate` bounds the count far
        // below u32, and an unvalidated revision fails to verify anyway.
        bytes.extend_from_slice(
            &u32::try_from(self.members.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for member in &self.members {
            bytes.extend_from_slice(&member.root_key);
            bytes.extend_from_slice(&member.device_log_hash);
        }
        bytes.extend_from_slice(&self.author_key);
        bytes
    }

    /// This revision's name, and what the next one must point at.
    ///
    /// Over the signature as well as the payload, so two revisions differing
    /// only in who signed them are different revisions — which is what makes
    /// a fork visible rather than a coincidence.
    #[must_use]
    pub fn hash(&self) -> RevisionHash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.signing_payload());
        hasher.update(&self.signature);
        *hasher.finalize().as_bytes()
    }

    /// Whether the key this revision names actually signed it.
    ///
    /// Says nothing about whether that key was allowed to: that needs the
    /// owner's device log.
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

    /// The log state the owner sealed this revision's key against for one
    /// member, or `None` if they are not a member of it.
    #[must_use]
    pub fn sealed_against(&self, member: &[u8; DEVICE_KEY_BYTES]) -> Option<LogHash> {
        self.members
            .iter()
            .find(|candidate| &candidate.root_key == member)
            .map(|candidate| candidate.device_log_hash)
    }

    /// The canonical bytes: the signing payload with the signature appended.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.signing_payload();
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    /// Reads canonical bytes back into a revision, and validates its shape.
    ///
    /// # Errors
    ///
    /// Returns [`RevisionError`] when the bytes are truncated, carry trailing
    /// data, declare another domain, or decode into something
    /// [`Self::validate`] refuses.
    pub fn decode(bytes: &[u8]) -> Result<Self, RevisionError> {
        let mut reader = Reader::new(bytes);
        if reader.take(DOMAIN.len())? != DOMAIN {
            return Err(RevisionError::UnknownDomain);
        }
        let collection_id = reader.array()?;
        let number = reader.u64()?;
        let previous_hash = reader.array()?;
        let manifest_hash = reader.array()?;
        let owner_root_key = reader.array()?;
        let at_unix_ns = reader.u64()?;

        let count = reader.u32()? as usize;
        if count > MAX_MEMBERS {
            return Err(RevisionError::TooManyMembers { actual: count });
        }
        let mut members = Vec::with_capacity(count);
        for _ in 0..count {
            members.push(Member {
                root_key: reader.array()?,
                device_log_hash: reader.array::<LOG_HASH_BYTES>()?,
            });
        }

        let revision = Self {
            collection_id,
            number,
            previous_hash,
            manifest_hash,
            owner_root_key,
            at_unix_ns,
            members,
            author_key: reader.array()?,
            signature: reader.array()?,
        };
        if !reader.is_empty() {
            return Err(RevisionError::Malformed);
        }
        revision.validate()?;
        Ok(revision)
    }
}

/// A cursor that refuses to read past the end rather than panicking.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RevisionError> {
        if self.bytes.len() < count {
            return Err(RevisionError::Malformed);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RevisionError> {
        let taken = self.take(N)?;
        <[u8; N]>::try_from(taken).map_err(|_| RevisionError::Malformed)
    }

    fn u32(&mut self) -> Result<u32, RevisionError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, RevisionError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    //! A revision is judged here only on what it says about itself. The rules
    //! that need a device log or a held state live in `client`, and their
    //! absence from these tests is the point: this crate cannot decide them,
    //! so it must not appear to.

    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
    const OWNER_SEED: [u8; 32] = [1; 32];
    const NOW: u64 = 1_700_000_000_000_000_000;

    fn key(seed: [u8; 32]) -> SigningKey {
        SigningKey::from_bytes(&seed)
    }

    fn member(root: u8) -> Member {
        Member {
            root_key: [root; DEVICE_KEY_BYTES],
            device_log_hash: [root.wrapping_add(0x80); LOG_HASH_BYTES],
        }
    }

    /// Revision one of a collection with two members, signed by its owner.
    fn first(owner: &SigningKey) -> Revision {
        sign(
            Revision {
                collection_id: COLLECTION,
                number: 1,
                previous_hash: NO_PREVIOUS,
                manifest_hash: [0x22; 32],
                owner_root_key: owner.verifying_key().to_bytes(),
                at_unix_ns: NOW,
                members: vec![member(2), member(3)],
                author_key: owner.verifying_key().to_bytes(),
                signature: [0; SIGNATURE_BYTES],
            },
            owner,
        )
    }

    fn sign(mut revision: Revision, author: &SigningKey) -> Revision {
        revision.signature = author.sign(&revision.signing_payload()).to_bytes();
        revision
    }

    #[test]
    fn a_revision_round_trips_through_its_canonical_bytes() {
        let owner = key(OWNER_SEED);
        let revision = first(&owner);

        let encoded = revision.encode();
        assert!(encoded.starts_with(DOMAIN));
        assert_eq!(Revision::decode(&encoded).expect("decodes"), revision);
        assert!(revision.verify());
        assert_eq!(revision.validate(), Ok(()));

        // The signature is the tail, and the payload is everything before it.
        assert_eq!(
            encoded.len(),
            revision.signing_payload().len() + SIGNATURE_BYTES
        );
    }

    #[test]
    fn a_second_revision_names_the_one_before_it() {
        let owner = key(OWNER_SEED);
        let previous = first(&owner);
        let second = sign(
            Revision {
                number: 2,
                previous_hash: previous.hash(),
                manifest_hash: [0x33; 32],
                ..previous.clone()
            },
            &owner,
        );

        assert_eq!(second.validate(), Ok(()));
        assert_eq!(second.previous_hash, previous.hash());
        assert_ne!(second.hash(), previous.hash());
        assert_eq!(Revision::decode(&second.encode()).expect("decodes"), second);
    }

    #[test]
    fn a_chain_must_begin_at_one_and_say_so() {
        let owner = key(OWNER_SEED);
        let revision = first(&owner);

        let unnumbered = Revision {
            number: 0,
            ..revision.clone()
        };
        assert_eq!(
            unnumbered.validate(),
            Err(RevisionError::NotNumbered { number: 0 })
        );

        // Revision one that claims a predecessor, and a later one that claims
        // none: both would let a chain be joined anywhere.
        let rooted_but_chained = Revision {
            previous_hash: [7; REVISION_HASH_BYTES],
            ..revision.clone()
        };
        assert_eq!(
            rooted_but_chained.validate(),
            Err(RevisionError::FirstFollowsSomething)
        );

        let orphan = Revision {
            number: 2,
            previous_hash: NO_PREVIOUS,
            ..revision
        };
        assert_eq!(
            orphan.validate(),
            Err(RevisionError::FollowsNothing { number: 2 })
        );
    }

    #[test]
    fn members_are_ascending_unique_and_bounded() {
        let owner = key(OWNER_SEED);
        let revision = first(&owner);

        let descending = Revision {
            members: vec![member(3), member(2)],
            ..revision.clone()
        };
        assert_eq!(
            descending.validate(),
            Err(RevisionError::MembersOutOfOrder { index: 1 })
        );

        let repeated = Revision {
            members: vec![member(2), member(2)],
            ..revision.clone()
        };
        assert_eq!(
            repeated.validate(),
            Err(RevisionError::DuplicateMember { index: 1 })
        );

        let crowded = Revision {
            members: (0..=MAX_MEMBERS)
                .map(|index| {
                    let mut root_key = [0; DEVICE_KEY_BYTES];
                    // Big-endian, so ascending indices are ascending keys and
                    // the count is what fails rather than the ordering.
                    root_key[..8].copy_from_slice(&(index as u64).to_be_bytes());
                    Member {
                        root_key,
                        device_log_hash: [0; LOG_HASH_BYTES],
                    }
                })
                .collect(),
            ..revision.clone()
        };
        assert_eq!(
            crowded.validate(),
            Err(RevisionError::TooManyMembers {
                actual: MAX_MEMBERS + 1
            })
        );

        // No members is a private collection, not a malformed one.
        let alone = Revision {
            members: Vec::new(),
            ..revision
        };
        assert_eq!(alone.validate(), Ok(()));
        assert_eq!(Revision::decode(&alone.encode()).expect("decodes"), alone);
    }

    #[test]
    fn a_forged_or_unusable_signature_does_not_verify() {
        let owner = key(OWNER_SEED);
        let stranger = key([9; 32]);
        let revision = first(&owner);

        let mut forged = revision.clone();
        forged.signature = stranger.sign(&revision.signing_payload()).to_bytes();
        assert!(!forged.verify(), "signed by someone other than the author");

        let off_curve = (2_u8..=u8::MAX)
            .map(|byte| [byte; DEVICE_KEY_BYTES])
            .find(|bytes| VerifyingKey::from_bytes(bytes).is_err())
            .expect("some byte pattern is not a curve point");
        let unusable = Revision {
            author_key: off_curve,
            ..revision.clone()
        };
        assert!(!unusable.verify());

        // Every field is covered, and the hash covers the signature too.
        let moved = Revision {
            at_unix_ns: revision.at_unix_ns + 1,
            ..revision.clone()
        };
        assert!(!moved.verify());
        assert_ne!(moved.hash(), revision.hash());

        let mut resigned = revision.clone();
        resigned.signature = [7; SIGNATURE_BYTES];
        assert_eq!(resigned.signing_payload(), revision.signing_payload());
        assert_ne!(
            resigned.hash(),
            revision.hash(),
            "the hash names the signature, so a fork is visible"
        );
    }

    #[test]
    fn truncated_trailing_and_foreign_bytes_are_refused() {
        let owner = key(OWNER_SEED);
        let encoded = first(&owner).encode();

        assert_eq!(Revision::decode(&[]), Err(RevisionError::Malformed));
        assert_eq!(
            Revision::decode(&encoded[..encoded.len() - 1]),
            Err(RevisionError::Malformed)
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(Revision::decode(&trailing), Err(RevisionError::Malformed));

        let mut foreign = encoded.clone();
        foreign[0] ^= 1;
        assert_eq!(
            Revision::decode(&foreign),
            Err(RevisionError::UnknownDomain)
        );

        // A member count larger than the bound is refused before allocating
        // for it, which is the whole reason the bound exists.
        let mut inflated = encoded.clone();
        let count_at = DOMAIN.len() + SHARE_ID_BYTES + 8 + 32 + 32 + 32 + 8;
        inflated[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            Revision::decode(&inflated),
            Err(RevisionError::TooManyMembers {
                actual: u32::MAX as usize
            })
        );

        // Decoding also enforces shape, not only length.
        let mut zero_numbered = first(&owner);
        zero_numbered.number = 0;
        assert_eq!(
            Revision::decode(&zero_numbered.encode()),
            Err(RevisionError::NotNumbered { number: 0 })
        );
    }

    #[test]
    fn a_revision_reports_what_it_sealed_against_for_each_member() {
        let owner = key(OWNER_SEED);
        let revision = first(&owner);

        assert_eq!(
            revision.sealed_against(&[2; DEVICE_KEY_BYTES]),
            Some(member(2).device_log_hash)
        );
        assert_eq!(revision.sealed_against(&[9; DEVICE_KEY_BYTES]), None);
    }
}
