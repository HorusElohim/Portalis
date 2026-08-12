//! Sealing a manifest into the sealed manifest Nexus stores and cannot read.
//!
//! This is `SPEC.md` §11. The server holds these bytes opaquely by design,
//! which means nothing on the server side can detect a client that builds
//! them differently — the format is a contract between clients, and this is
//! the only implementation of it.
//!
//! The nonce is derived from the share and revision rather than drawn at
//! random. It is unique for every revision under one key, which is what a
//! fixed-nonce AEAD needs, and it makes sealing deterministic: a publisher
//! whose acknowledgement was lost re-seals to the same bytes, and Nexus
//! recognises the identical retry instead of refusing a second revision.

use thiserror::Error;

use crate::SHARE_ID_BYTES;
use crate::format::aead::{self, AeadError, ContentKey};
use crate::format::manifest::{Manifest, ManifestEntry, ManifestHash};

/// Mixed into the nonce derivation, so these bytes cannot collide with a
/// digest computed anywhere else for the same share and revision.
const NONCE_CONTEXT: &[u8] = b"portalis.manifest.v1/nonce";

/// The encoding a sealed manifest declares in its first byte.
pub const SEALED_MANIFEST_VERSION: u8 = 1;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SealedManifestError {
    /// Truncated, sealed under another version, or refused by the AEAD —
    /// which covers a wrong key, a wrong collection, a wrong revision and
    /// tampered bytes alike, because telling those apart would say more about
    /// the ciphertext than a failed open should.
    #[error(transparent)]
    Sealed(#[from] AeadError),
    /// It opened, and what came out is not a canonical manifest. Only a
    /// holder of the content key can cause this, so it means a peer built
    /// something wrong rather than an attacker guessing.
    #[error("the sealed value opened but does not hold a canonical manifest")]
    Malformed,
}

/// What a sealed manifest is bound to, and what fails to open it under anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestContext {
    pub collection_id: [u8; SHARE_ID_BYTES],
    pub revision: u64,
    pub manifest_hash: ManifestHash,
}

impl ManifestContext {
    /// The associated data: a sealed manifest lifted onto another share, revision, or
    /// snapshot fails to open rather than decrypting into the wrong place.
    fn associated_data(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SHARE_ID_BYTES + 8 + self.manifest_hash.len());
        bytes.extend_from_slice(&self.collection_id);
        bytes.extend_from_slice(&self.revision.to_le_bytes());
        bytes.extend_from_slice(&self.manifest_hash);
        bytes
    }

    fn nonce(&self) -> [u8; aead::NONCE_BYTES] {
        aead::derived_nonce(
            NONCE_CONTEXT,
            &[
                &self.collection_id,
                &self.revision.to_le_bytes(),
                &self.manifest_hash,
            ],
        )
    }
}

/// Seals `manifest` for one revision of one share.
///
/// The context's `manifest_hash` must be the manifest's own content root; it is
/// taken from the manifest rather than the caller so the two cannot disagree.
///
/// # Errors
///
/// Never fails in practice: `ChaCha20Poly1305` refuses only plaintexts far
/// larger than a manifest can be, and that is reported rather than panicked
/// on.
pub fn seal(
    key: &ContentKey,
    collection_id: [u8; SHARE_ID_BYTES],
    revision: u64,
    manifest: &Manifest,
) -> Result<Vec<u8>, SealedManifestError> {
    let context = ManifestContext {
        collection_id,
        revision,
        manifest_hash: manifest.hash(),
    };
    Ok(aead::seal(
        key,
        SEALED_MANIFEST_VERSION,
        context.nonce(),
        &context.associated_data(),
        &manifest.encode(),
    )?)
}

/// Opens a sealed manifest and returns the manifest inside it.
///
/// # Errors
///
/// Returns [`SealedManifestError`] when the sealed manifest is truncated, declares a version
/// this client does not speak, does not open under this key and context, or
/// holds something that is not a canonical manifest.
pub fn open(
    key: &ContentKey,
    context: &ManifestContext,
    sealed: &[u8],
) -> Result<Manifest, SealedManifestError> {
    // The nonce is derived, not trusted: a value carrying another nonce was
    // not sealed for this collection and revision, and is refused before any
    // decryption is attempted.
    let plaintext = aead::open_derived(
        key,
        SEALED_MANIFEST_VERSION,
        context.nonce(),
        &context.associated_data(),
        sealed,
    )?;

    let manifest = decode(&plaintext)?;
    // The content root is authenticated as associated data, so this can only
    // disagree if a sealed manifest was sealed against a root that was not its own.
    if manifest.hash() != context.manifest_hash {
        return Err(SealedManifestError::Malformed);
    }
    Ok(manifest)
}

/// Reads canonical manifest bytes back into entries.
fn decode(bytes: &[u8]) -> Result<Manifest, SealedManifestError> {
    let mut reader = Reader::new(bytes);
    reader.expect_domain()?;
    let count = reader.u32()? as usize;
    let mut entries = Vec::with_capacity(count.min(crate::format::manifest::MAX_ENTRIES));
    for _ in 0..count {
        entries.push(reader.entry()?);
    }
    if !reader.is_empty() {
        return Err(SealedManifestError::Malformed);
    }
    Manifest::new(entries).map_err(|_| SealedManifestError::Malformed)
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], SealedManifestError> {
        if self.bytes.len() < count {
            return Err(SealedManifestError::Malformed);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SealedManifestError> {
        let taken = self.take(N)?;
        <[u8; N]>::try_from(taken).map_err(|_| SealedManifestError::Malformed)
    }

    fn byte(&mut self) -> Result<u8, SealedManifestError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, SealedManifestError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, SealedManifestError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn expect_domain(&mut self) -> Result<(), SealedManifestError> {
        let expected = Manifest::default().encode();
        let domain = &expected[..expected.len() - 4];
        if self.take(domain.len())? == domain {
            Ok(())
        } else {
            Err(SealedManifestError::Malformed)
        }
    }

    fn entry(&mut self) -> Result<ManifestEntry, SealedManifestError> {
        if self.byte()? != crate::format::manifest::ENTRY_VERSION {
            return Err(SealedManifestError::Malformed);
        }
        let info_hash = self.array()?;
        let name_len = self.u32()? as usize;
        let name = String::from_utf8(self.take(name_len)?.to_vec())
            .map_err(|_| SealedManifestError::Malformed)?;
        let thumbnail_hash = match self.byte()? {
            0 => None,
            1 => Some(self.array()?),
            _ => return Err(SealedManifestError::Malformed),
        };
        Ok(ManifestEntry {
            info_hash,
            name,
            thumbnail_hash,
            author_public_key: self.array()?,
            added_at_unix_ns: self.u64()?,
            signature: self.array()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::SIGNATURE_BYTES;
    use crate::format::aead::CONTENT_KEY_BYTES;
    use crate::format::manifest::{INFO_HASH_BYTES, THUMBNAIL_HASH_BYTES};

    const KEY: ContentKey = [3; CONTENT_KEY_BYTES];
    const SHARE: [u8; SHARE_ID_BYTES] = [5; SHARE_ID_BYTES];

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn entry(info_hash: u8, name: &str, thumbnail: bool) -> ManifestEntry {
        let signing_key = signing_key();
        let mut entry = ManifestEntry {
            info_hash: [info_hash; INFO_HASH_BYTES],
            name: name.to_owned(),
            thumbnail_hash: thumbnail.then_some([4; THUMBNAIL_HASH_BYTES]),
            author_public_key: signing_key.verifying_key().to_bytes(),
            added_at_unix_ns: 1_700_000_000_000_000_000,
            signature: [0; SIGNATURE_BYTES],
        };
        entry.signature = signing_key.sign(&entry.signing_payload()).to_bytes();
        entry
    }

    fn manifest() -> Manifest {
        Manifest::new(vec![
            entry(1, "one.jpg", false),
            entry(2, "two.mp4", true),
            entry(3, "a name with spaces and é", false),
        ])
        .expect("built")
    }

    fn context(manifest: &Manifest, revision: u64) -> ManifestContext {
        ManifestContext {
            collection_id: SHARE,
            revision,
            manifest_hash: manifest.hash(),
        }
    }

    #[test]
    fn a_sealed_manifest_opens_back_into_the_same_entries() {
        let manifest = manifest();
        let sealed = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        let opened = open(&KEY, &context(&manifest, 1), &sealed).expect("opened");

        assert_eq!(opened, manifest);
        assert_eq!(opened.hash(), manifest.hash());
        assert_eq!(sealed[0], SEALED_MANIFEST_VERSION);
    }

    #[test]
    fn an_empty_manifest_round_trips_too() {
        let manifest = Manifest::default();
        let sealed = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        assert_eq!(
            open(&KEY, &context(&manifest, 1), &sealed).expect("opened"),
            manifest
        );
    }

    /// A publisher whose acknowledgement was lost re-seals and retries. The
    /// bytes have to match, or Nexus would see a second revision where the
    /// publisher meant a retry.
    #[test]
    fn sealing_the_same_revision_twice_produces_the_same_bytes() {
        let manifest = manifest();

        assert_eq!(
            seal(&KEY, SHARE, 7, &manifest).expect("sealed"),
            seal(&KEY, SHARE, 7, &manifest).expect("sealed again")
        );
        assert_ne!(
            seal(&KEY, SHARE, 7, &manifest).expect("sealed"),
            seal(&KEY, SHARE, 8, &manifest).expect("next revision"),
            "a different revision is a different nonce"
        );
    }

    #[test]
    fn a_different_snapshot_at_the_same_revision_gets_a_different_nonce() {
        let first = manifest();
        let second = Manifest::new(vec![entry(1, "changed.jpg", false)]).expect("built");

        assert_ne!(
            seal(&KEY, SHARE, 7, &first).expect("sealed"),
            seal(&KEY, SHARE, 7, &second).expect("sealed candidate"),
            "different candidate plaintexts must not reuse a key and nonce"
        );
    }

    /// The context is authenticated, so a sealed manifest cannot be lifted from the
    /// share, revision, or snapshot it was sealed for onto another.
    #[test]
    fn a_sealed_manifest_does_not_open_anywhere_it_was_not_sealed() {
        let manifest = manifest();
        let sealed = seal(&KEY, SHARE, 1, &manifest).expect("sealed");
        let correct = context(&manifest, 1);

        for wrong in [
            ManifestContext {
                collection_id: [6; SHARE_ID_BYTES],
                ..correct
            },
            ManifestContext {
                revision: 2,
                ..correct
            },
            ManifestContext {
                manifest_hash: [0; 32],
                ..correct
            },
        ] {
            assert_eq!(
                open(&KEY, &wrong, &sealed),
                Err(SealedManifestError::Sealed(AeadError::Rejected)),
                "a sealed is bound to exactly one place"
            );
        }

        assert_eq!(
            open(&[4; CONTENT_KEY_BYTES], &correct, &sealed),
            Err(SealedManifestError::Sealed(AeadError::Rejected)),
            "and to exactly one key"
        );
    }

    #[test]
    fn a_tampered_sealed_manifest_is_refused() {
        let manifest = manifest();
        let correct = context(&manifest, 1);
        let sealed = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        assert_eq!(
            open(&KEY, &correct, &flipped),
            Err(SealedManifestError::Sealed(AeadError::Rejected))
        );

        let mut wrong_nonce = sealed.clone();
        wrong_nonce[1] ^= 0x01;
        assert_eq!(
            open(&KEY, &correct, &wrong_nonce),
            Err(SealedManifestError::Sealed(AeadError::Rejected)),
            "the nonce is derived, so a sealed carrying another is not ours"
        );

        assert_eq!(
            open(&KEY, &correct, &sealed[..sealed.len() - 1]),
            Err(SealedManifestError::Sealed(AeadError::Rejected)),
            "a truncated ciphertext fails its tag"
        );
    }

    #[test]
    fn a_sealed_manifest_too_short_or_too_new_is_refused_before_anything_else() {
        let manifest = manifest();
        let correct = context(&manifest, 1);

        assert_eq!(
            open(&KEY, &correct, &[]),
            Err(SealedManifestError::Sealed(AeadError::TooShort {
                actual: 0
            }))
        );
        assert_eq!(
            open(&KEY, &correct, &[SEALED_MANIFEST_VERSION, 1, 2]),
            Err(SealedManifestError::Sealed(AeadError::TooShort {
                actual: 3
            }))
        );
        assert_eq!(
            open(&KEY, &correct, &[SEALED_MANIFEST_VERSION + 1; 64]),
            Err(SealedManifestError::Sealed(AeadError::UnsupportedVersion {
                expected: SEALED_MANIFEST_VERSION,
                actual: SEALED_MANIFEST_VERSION + 1
            })),
            "a version this client does not speak is not guessed at"
        );
    }

    /// Anything that opens but is not a canonical manifest is refused rather
    /// than half-read, which is what stops a peer with the share key from
    /// feeding a client arbitrary structure.
    #[test]
    fn plaintext_that_is_not_a_manifest_is_refused() {
        let manifest = manifest();
        let real = manifest.encode();

        for corrupt in [
            Vec::new(),
            b"portalis.manifest.v2\0".to_vec(),
            real[..real.len() - 1].to_vec(),
            [real.clone(), vec![0]].concat(),
            {
                // An entry count larger than the entries that follow.
                let mut bytes = real.clone();
                bytes[21] = 0xff;
                bytes
            },
            {
                // An entry declaring a version this client does not write.
                let mut bytes = real.clone();
                bytes[25] = 2;
                bytes
            },
            {
                // A thumbnail flag that is neither present nor absent.
                let mut bytes = real.clone();
                let flag = 25 + 1 + INFO_HASH_BYTES + 4 + "one.jpg".len();
                bytes[flag] = 2;
                bytes
            },
            {
                // A name that is not UTF-8.
                let mut bytes = real.clone();
                bytes[25 + 1 + INFO_HASH_BYTES + 4] = 0xff;
                bytes
            },
        ] {
            let sealed = seal_raw(&KEY, &context(&manifest, 1), &corrupt);
            assert_eq!(
                open(&KEY, &context(&manifest, 1), &sealed),
                Err(SealedManifestError::Malformed)
            );
        }
    }

    /// A sealed manifest whose plaintext is a valid manifest, but not the one the
    /// context names, is refused: the root is what a revision points at.
    #[test]
    fn a_manifest_that_is_not_the_one_named_is_refused() {
        let manifest = manifest();
        let other = Manifest::new(vec![entry(9, "elsewhere", false)]).expect("built");

        let sealed = seal_raw(&KEY, &context(&manifest, 1), &other.encode());

        assert_eq!(
            open(&KEY, &context(&manifest, 1), &sealed),
            Err(SealedManifestError::Malformed)
        );
    }

    /// Seals bytes that [`seal`] would never produce, so the reader can be
    /// tested against plaintext a hostile holder of the key could write.
    fn seal_raw(key: &ContentKey, context: &ManifestContext, plaintext: &[u8]) -> Vec<u8> {
        aead::seal(
            key,
            SEALED_MANIFEST_VERSION,
            context.nonce(),
            &context.associated_data(),
            plaintext,
        )
        .expect("test plaintext seals")
    }
}
