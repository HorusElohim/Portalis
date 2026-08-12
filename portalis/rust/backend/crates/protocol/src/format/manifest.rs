//! The canonical manifest, and the content root taken over it.
//!
//! This is `SPEC.md` §11. It lives in the client crate because Nexus never
//! sees it: the server stores a capsule it cannot open and a `ManifestHash` it
//! cannot recompute, so nothing on the server side can catch two clients that
//! disagree about a byte. One implementation, shared by every platform, is
//! what makes the agreement hold.
//!
//! Every integer is little-endian and every variable-length field carries its
//! length first, so no pair of fields can be reinterpreted as a different
//! pair. That is the same rule the signing payloads in `portalis-nexus-
//! protocol` follow, and it is what the Portalis application's own manifest
//! entries lacked: a name written directly before an optional thumbnail could
//! be read as a longer name and no thumbnail.

use crate::{DEVICE_KEY_BYTES, SIGNATURE_BYTES, SNAPSHOT_ID_BYTES};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

/// Prefix bound into every canonical manifest, so bytes hashed here can never
/// collide with bytes hashed for another purpose.
const MANIFEST_DOMAIN: &[u8] = b"portalis.manifest.v1\0";

/// The encoding version each entry declares.
pub const ENTRY_VERSION: u8 = 1;

/// A `BitTorrent` v1 info hash, which is what names a manifest entry.
pub const INFO_HASH_BYTES: usize = 20;
/// A `BLAKE3` thumbnail digest.
pub const THUMBNAIL_HASH_BYTES: usize = 32;

/// The `BLAKE3` content root of a resolved canonical manifest.
pub type ManifestHash = [u8; SNAPSHOT_ID_BYTES];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    /// Two entries naming one info hash would make the ordering ambiguous and
    /// the share self-contradictory: the same torrent under two names.
    #[error("info hash {} appears more than once", hex(.info_hash))]
    DuplicateEntry { info_hash: [u8; INFO_HASH_BYTES] },
    /// Names cross the wire inside a capsule and are rendered by every
    /// client, so a length that cannot be encoded is refused before hashing.
    #[error("entry name is {actual} bytes, over the {MAX_ENTRY_NAME_BYTES}-byte limit")]
    NameTooLong { actual: usize },
    #[error("entry name is not NFC-normalized")]
    NameNotNfc,
    #[error("a manifest holds at most {MAX_ENTRIES} entries, got {actual}")]
    TooManyEntries { actual: usize },
    #[error("entry for info hash {} has an invalid signature", hex(.info_hash))]
    InvalidSignature { info_hash: [u8; INFO_HASH_BYTES] },
}

/// The per-snapshot entry bound from §8's quota, so one manifest cannot grow
/// past what a capsule and a frame can carry.
pub const MAX_ENTRIES: usize = 4_096;
/// A generous bound on a single media name, well past any real file name.
pub const MAX_ENTRY_NAME_BYTES: usize = 1_024;

/// One media item, as it is hashed and encrypted.
///
/// The author's Ed25519 public key travels rather than an identifier derived
/// from it: the key is what the signature verifies against, and both the
/// application's device identifier and Nexus's are derivable from it, so one
/// entry satisfies both without a lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestEntry {
    pub info_hash: [u8; INFO_HASH_BYTES],
    pub name: String,
    pub thumbnail_hash: Option<[u8; THUMBNAIL_HASH_BYTES]>,
    pub author_public_key: [u8; DEVICE_KEY_BYTES],
    pub added_at_unix_ns: u64,
    pub signature: [u8; SIGNATURE_BYTES],
}

impl ManifestEntry {
    /// Returns the representation that must be signed and placed on the wire.
    ///
    /// Callers normalize before constructing/signing an entry. A decoder does
    /// not normalize authenticated bytes after the fact; it rejects a name
    /// that is not already in this form.
    #[must_use]
    pub fn normalize_name(name: &str) -> String {
        name.nfc().collect()
    }

    /// Everything except the signature: exactly what a signature covers.
    ///
    /// Split out so a client signs and verifies the same bytes it hashes,
    /// rather than two encodings that have to be kept in step.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.encode_signed_fields(&mut bytes);
        bytes
    }

    /// Verifies the signature against the public key carried by this entry.
    #[must_use]
    pub fn verify(&self) -> bool {
        let Ok(public_key) = VerifyingKey::from_bytes(&self.author_public_key) else {
            return false;
        };
        let signature = Signature::from_bytes(&self.signature);
        public_key
            .verify(self.signing_payload().as_slice(), &signature)
            .is_ok()
    }

    fn encode_signed_fields(&self, bytes: &mut Vec<u8>) {
        bytes.push(ENTRY_VERSION);
        bytes.extend_from_slice(&self.info_hash);
        let name = self.name.as_bytes();
        bytes.extend_from_slice(&length(name.len()));
        bytes.extend_from_slice(name);
        match &self.thumbnail_hash {
            Some(hash) => {
                bytes.push(1);
                bytes.extend_from_slice(hash);
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&self.author_public_key);
        bytes.extend_from_slice(&self.added_at_unix_ns.to_le_bytes());
    }
}

/// The media a share contains at one revision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// Builds a manifest, ordering entries by info hash.
    ///
    /// Callers hand over whatever order they hold; the canonical order is
    /// this type's business, because a client that sorted differently would
    /// compute a different content root for the same media.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when an info hash repeats, a name is too
    /// long or not NFC-normalized, an entry signature is invalid, or there
    /// are more entries than a manifest may carry.
    pub fn new(mut entries: Vec<ManifestEntry>) -> Result<Self, ManifestError> {
        if entries.len() > MAX_ENTRIES {
            return Err(ManifestError::TooManyEntries {
                actual: entries.len(),
            });
        }
        for entry in &entries {
            if entry.name.len() > MAX_ENTRY_NAME_BYTES {
                return Err(ManifestError::NameTooLong {
                    actual: entry.name.len(),
                });
            }
            if ManifestEntry::normalize_name(&entry.name) != entry.name {
                return Err(ManifestError::NameNotNfc);
            }
            if !entry.verify() {
                return Err(ManifestError::InvalidSignature {
                    info_hash: entry.info_hash,
                });
            }
        }

        entries.sort_unstable_by_key(|entry| entry.info_hash);
        if let Some(pair) = entries
            .windows(2)
            .find(|pair| pair[0].info_hash == pair[1].info_hash)
        {
            return Err(ManifestError::DuplicateEntry {
                info_hash: pair[0].info_hash,
            });
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The canonical bytes: what gets hashed, and what the sealed form encrypts.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::from(MANIFEST_DOMAIN);
        bytes.extend_from_slice(&length(self.entries.len()));
        for entry in &self.entries {
            entry.encode_signed_fields(&mut bytes);
            bytes.extend_from_slice(&entry.signature);
        }
        bytes
    }

    /// The content root: `BLAKE3` over [`Manifest::encode`].
    #[must_use]
    pub fn hash(&self) -> ManifestHash {
        *blake3::hash(&self.encode()).as_bytes()
    }
}

/// A length prefix, as the four little-endian bytes the encoding names.
///
/// Every caller has already bounded what it is measuring — entry counts
/// against [`MAX_ENTRIES`] and names against [`MAX_ENTRY_NAME_BYTES`], both
/// far inside `u32` — so this saturates rather than carrying an error case
/// no input can reach.
fn length(value: usize) -> [u8; 4] {
    u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::new(), |mut rendered, byte| {
        let _ = write!(rendered, "{byte:02x}");
        rendered
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn signed(mut entry: ManifestEntry, seed: u8) -> ManifestEntry {
        let signing_key = key(seed);
        entry.author_public_key = signing_key.verifying_key().to_bytes();
        entry.signature = signing_key.sign(&entry.signing_payload()).to_bytes();
        entry
    }

    fn entry(info_hash: u8, name: &str) -> ManifestEntry {
        signed(
            ManifestEntry {
                info_hash: [info_hash; INFO_HASH_BYTES],
                name: name.to_owned(),
                thumbnail_hash: None,
                author_public_key: [0; DEVICE_KEY_BYTES],
                added_at_unix_ns: 1_700_000_000_000_000_000,
                signature: [0; SIGNATURE_BYTES],
            },
            7,
        )
    }

    #[test]
    fn the_order_entries_arrive_in_does_not_change_the_content_root() {
        let ascending = Manifest::new(vec![entry(1, "one"), entry(2, "two"), entry(3, "three")])
            .expect("built");
        let shuffled = Manifest::new(vec![entry(3, "three"), entry(1, "one"), entry(2, "two")])
            .expect("built");

        assert_eq!(ascending.encode(), shuffled.encode());
        assert_eq!(ascending.hash(), shuffled.hash());
        assert_eq!(
            ascending
                .entries()
                .iter()
                .map(|entry| entry.info_hash[0])
                .collect::<Vec<_>>(),
            vec![1, 2, 3],
            "entries are stored in canonical order, not the order given"
        );
    }

    /// Adding, removing, renaming, or replacing media has to move the root,
    /// or a share could change without its revision saying so.
    #[test]
    fn every_meaningful_difference_changes_the_content_root() {
        let base = Manifest::new(vec![entry(1, "one")]).expect("built");
        let with_thumbnail = signed(
            ManifestEntry {
                thumbnail_hash: Some([4; THUMBNAIL_HASH_BYTES]),
                ..entry(1, "one")
            },
            7,
        );

        let roots = [
            base.hash(),
            Manifest::new(vec![entry(1, "renamed")])
                .expect("built")
                .hash(),
            Manifest::new(vec![entry(2, "one")]).expect("built").hash(),
            Manifest::new(vec![entry(1, "one"), entry(2, "two")])
                .expect("built")
                .hash(),
            Manifest::new(vec![with_thumbnail]).expect("built").hash(),
            Manifest::new(vec![signed(
                ManifestEntry {
                    added_at_unix_ns: 1,
                    ..entry(1, "one")
                },
                7,
            )])
            .expect("built")
            .hash(),
            Manifest::new(vec![signed(ManifestEntry { ..entry(1, "one") }, 8)])
                .expect("built")
                .hash(),
            Manifest::default().hash(),
        ];

        for (index, root) in roots.iter().enumerate() {
            for (other, another) in roots.iter().enumerate().skip(index + 1) {
                assert_ne!(root, another, "roots {index} and {other} collide");
            }
        }
    }

    /// The ambiguity the Portalis application's own entries carried: a name
    /// written straight before an optional thumbnail can be read as a longer
    /// name and no thumbnail. Length prefixes are what remove it.
    #[test]
    fn a_name_cannot_be_confused_with_the_field_after_it() {
        let thumbnail = [0x61; THUMBNAIL_HASH_BYTES]; // "aaaa…" as UTF-8.
        let with_thumbnail = Manifest::new(vec![signed(
            ManifestEntry {
                thumbnail_hash: Some(thumbnail),
                ..entry(1, "photo")
            },
            7,
        )])
        .expect("built");
        let swallowed = Manifest::new(vec![signed(
            ManifestEntry {
                name: format!("photo{}", "a".repeat(THUMBNAIL_HASH_BYTES)),
                thumbnail_hash: None,
                ..entry(1, "photo")
            },
            7,
        )])
        .expect("built");

        assert_ne!(with_thumbnail.encode(), swallowed.encode());
        assert_ne!(with_thumbnail.hash(), swallowed.hash());
    }

    /// A signature covers the entry and not the manifest around it, so an
    /// entry can be verified wherever it is carried.
    #[test]
    fn an_entry_signs_everything_about_itself_except_its_signature() {
        let entry = entry(1, "one");
        let payload = entry.signing_payload();

        assert_eq!(payload[0], ENTRY_VERSION);
        assert!(
            !payload
                .windows(SIGNATURE_BYTES)
                .any(|window| window == entry.signature),
            "the signature is not part of what it signs"
        );
        assert_eq!(
            payload,
            ManifestEntry {
                signature: [3; SIGNATURE_BYTES],
                ..entry.clone()
            }
            .signing_payload(),
            "and changing it does not change the payload"
        );
        assert_ne!(
            payload,
            ManifestEntry {
                name: "two".to_owned(),
                ..entry
            }
            .signing_payload()
        );
    }

    /// A key that is not a curve point verifies nothing, and says so rather
    /// than panicking on the way.
    #[test]
    fn an_entry_with_an_unusable_author_key_does_not_verify() {
        // Found rather than guessed: not every byte pattern is off the
        // curve, and a hard-coded one that happens to decompress would test
        // the signature check instead of the key check.
        let off_curve = (2_u8..=u8::MAX)
            .map(|byte| [byte; DEVICE_KEY_BYTES])
            .find(|bytes| VerifyingKey::from_bytes(bytes).is_err())
            .expect("some byte pattern is not a curve point");
        let unusable = ManifestEntry {
            author_public_key: off_curve,
            ..entry(1, "one")
        };

        assert!(!unusable.verify());
        assert_eq!(
            Manifest::new(vec![unusable]),
            Err(ManifestError::InvalidSignature {
                info_hash: [1; INFO_HASH_BYTES]
            })
        );
    }

    #[test]
    fn a_manifest_refuses_what_it_cannot_encode() {
        assert_eq!(
            Manifest::new(vec![entry(1, "one"), entry(1, "again")]),
            Err(ManifestError::DuplicateEntry {
                info_hash: [1; INFO_HASH_BYTES]
            })
        );
        assert_eq!(
            Manifest::new(vec![entry(1, &"n".repeat(MAX_ENTRY_NAME_BYTES + 1))]),
            Err(ManifestError::NameTooLong {
                actual: MAX_ENTRY_NAME_BYTES + 1
            })
        );
        assert!(Manifest::new(vec![entry(1, &"n".repeat(MAX_ENTRY_NAME_BYTES))]).is_ok());

        assert_eq!(
            Manifest::new(vec![entry(1, "e\u{301}")]),
            Err(ManifestError::NameNotNfc)
        );
        assert_eq!(ManifestEntry::normalize_name("e\u{301}"), "é");

        let mut forged = entry(1, "one");
        forged.signature[0] ^= 1;
        assert_eq!(
            Manifest::new(vec![forged]),
            Err(ManifestError::InvalidSignature {
                info_hash: [1; INFO_HASH_BYTES]
            })
        );

        let crowd: Vec<_> = (0..=MAX_ENTRIES)
            .map(|index| {
                signed(
                    ManifestEntry {
                        info_hash: index_hash(index),
                        ..entry(0, "many")
                    },
                    7,
                )
            })
            .collect();
        assert_eq!(
            Manifest::new(crowd.clone()),
            Err(ManifestError::TooManyEntries {
                actual: MAX_ENTRIES + 1
            })
        );
        assert_eq!(
            Manifest::new(crowd[..MAX_ENTRIES].to_vec())
                .expect("at the limit")
                .len(),
            MAX_ENTRIES
        );
    }

    #[test]
    fn an_empty_manifest_is_still_a_manifest() {
        let empty = Manifest::default();

        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.entries(), &[]);
        assert_eq!(
            empty.encode().len(),
            MANIFEST_DOMAIN.len() + 4,
            "the domain tag and a zero count"
        );
        assert_eq!(
            Manifest::new(Vec::new()).expect("built"),
            empty,
            "however it is built"
        );
    }

    /// The error names the hash a human has to go and look for.
    #[test]
    fn a_duplicate_is_reported_with_the_hash_that_repeated() {
        let error = Manifest::new(vec![entry(0xab, "one"), entry(0xab, "two")])
            .expect_err("duplicate refused");

        assert!(error.to_string().contains(&"ab".repeat(INFO_HASH_BYTES)));
    }

    fn index_hash(index: usize) -> [u8; INFO_HASH_BYTES] {
        let mut hash = [0; INFO_HASH_BYTES];
        let bytes = u32::try_from(index)
            .expect("test indices fit")
            .to_le_bytes();
        hash[..bytes.len()].copy_from_slice(&bytes);
        hash
    }
}
