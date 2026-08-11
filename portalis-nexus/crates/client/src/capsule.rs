//! Sealing a manifest into the capsule Nexus stores and cannot read.
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

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use portalis_nexus_protocol::SHARE_ID_BYTES;
use thiserror::Error;

use crate::manifest::{Manifest, ManifestEntry, SnapshotId};

/// Mixed into the nonce derivation, so these bytes cannot collide with a
/// digest computed anywhere else for the same share and revision.
const NONCE_CONTEXT: &[u8] = b"portalis.capsule.v1/nonce";

/// The encoding a capsule declares in its first byte.
pub const CAPSULE_VERSION: u8 = 1;
/// A share's symmetric key: the secret Nexus never receives.
pub const SHARE_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

pub type ShareKey = [u8; SHARE_KEY_BYTES];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CapsuleError {
    #[error("capsule is {actual} bytes, too short to hold a version and nonce")]
    TooShort { actual: usize },
    /// The only field a reader may act on before authenticating the rest, so
    /// an unknown one is refused rather than guessed at.
    #[error("capsule declares version {actual}, and this client speaks {CAPSULE_VERSION}")]
    UnsupportedVersion { actual: u8 },
    /// Wrong key, wrong share, wrong revision, or tampered bytes — all of
    /// which are one answer, because distinguishing them would say more about
    /// the ciphertext than a failed open should.
    #[error("the capsule did not open")]
    Rejected,
    #[error("the capsule opened but does not hold a canonical manifest")]
    Malformed,
}

/// What a capsule is bound to, and what fails to open it under anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapsuleContext {
    pub share_id: [u8; SHARE_ID_BYTES],
    pub revision: u64,
    pub snapshot_id: SnapshotId,
}

impl CapsuleContext {
    /// The associated data: a capsule lifted onto another share, revision, or
    /// snapshot fails to open rather than decrypting into the wrong place.
    fn associated_data(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SHARE_ID_BYTES + 8 + self.snapshot_id.len());
        bytes.extend_from_slice(&self.share_id);
        bytes.extend_from_slice(&self.revision.to_le_bytes());
        bytes.extend_from_slice(&self.snapshot_id);
        bytes
    }

    fn nonce(&self) -> Nonce {
        let mut hasher = blake3::Hasher::new();
        hasher.update(NONCE_CONTEXT);
        hasher.update(&self.share_id);
        hasher.update(&self.revision.to_le_bytes());
        let digest = hasher.finalize();
        let mut nonce = [0_u8; NONCE_BYTES];
        nonce.copy_from_slice(&digest.as_bytes()[..NONCE_BYTES]);
        Nonce::from(nonce)
    }
}

/// Seals `manifest` for one revision of one share.
///
/// The context's `snapshot_id` must be the manifest's own content root; it is
/// taken from the manifest rather than the caller so the two cannot disagree.
///
/// # Errors
///
/// Never fails in practice: `ChaCha20Poly1305` refuses only plaintexts far
/// larger than a manifest can be, and that is reported rather than panicked
/// on.
pub fn seal(
    key: &ShareKey,
    share_id: [u8; SHARE_ID_BYTES],
    revision: u64,
    manifest: &Manifest,
) -> Result<Vec<u8>, CapsuleError> {
    let context = CapsuleContext {
        share_id,
        revision,
        snapshot_id: manifest.snapshot_id(),
    };
    let plaintext = manifest.encode();
    let ciphertext = ChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(
            &context.nonce(),
            Payload {
                msg: &plaintext,
                aad: &context.associated_data(),
            },
        )
        .map_err(|_| CapsuleError::Rejected)?;

    let mut capsule = Vec::with_capacity(1 + NONCE_BYTES + ciphertext.len());
    capsule.push(CAPSULE_VERSION);
    capsule.extend_from_slice(context.nonce().as_slice());
    capsule.extend_from_slice(&ciphertext);
    Ok(capsule)
}

/// Opens a capsule and returns the manifest inside it.
///
/// # Errors
///
/// Returns [`CapsuleError`] when the capsule is truncated, declares a version
/// this client does not speak, does not open under this key and context, or
/// holds something that is not a canonical manifest.
pub fn open(
    key: &ShareKey,
    context: &CapsuleContext,
    capsule: &[u8],
) -> Result<Manifest, CapsuleError> {
    let (&version, rest) = capsule.split_first().ok_or(CapsuleError::TooShort {
        actual: capsule.len(),
    })?;
    if version != CAPSULE_VERSION {
        return Err(CapsuleError::UnsupportedVersion { actual: version });
    }
    if rest.len() < NONCE_BYTES {
        return Err(CapsuleError::TooShort {
            actual: capsule.len(),
        });
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_BYTES);

    // The nonce is derived, not trusted: a capsule carrying someone else's
    // nonce is one that was not sealed for this share and revision.
    if nonce != context.nonce().as_slice() {
        return Err(CapsuleError::Rejected);
    }

    let plaintext = ChaCha20Poly1305::new(&Key::from(*key))
        .decrypt(
            &context.nonce(),
            Payload {
                msg: ciphertext,
                aad: &context.associated_data(),
            },
        )
        .map_err(|_| CapsuleError::Rejected)?;

    let manifest = decode(&plaintext)?;
    // The content root is authenticated as associated data, so this can only
    // disagree if a capsule was sealed against a root that was not its own.
    if manifest.snapshot_id() != context.snapshot_id {
        return Err(CapsuleError::Malformed);
    }
    Ok(manifest)
}

/// Reads canonical manifest bytes back into entries.
fn decode(bytes: &[u8]) -> Result<Manifest, CapsuleError> {
    let mut reader = Reader::new(bytes);
    reader.expect_domain()?;
    let count = reader.u32()? as usize;
    let mut entries = Vec::with_capacity(count.min(crate::manifest::MAX_ENTRIES));
    for _ in 0..count {
        entries.push(reader.entry()?);
    }
    if !reader.is_empty() {
        return Err(CapsuleError::Malformed);
    }
    Manifest::new(entries).map_err(|_| CapsuleError::Malformed)
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], CapsuleError> {
        if self.bytes.len() < count {
            return Err(CapsuleError::Malformed);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CapsuleError> {
        let taken = self.take(N)?;
        <[u8; N]>::try_from(taken).map_err(|_| CapsuleError::Malformed)
    }

    fn byte(&mut self) -> Result<u8, CapsuleError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, CapsuleError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CapsuleError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn expect_domain(&mut self) -> Result<(), CapsuleError> {
        let expected = Manifest::default().encode();
        let domain = &expected[..expected.len() - 4];
        if self.take(domain.len())? == domain {
            Ok(())
        } else {
            Err(CapsuleError::Malformed)
        }
    }

    fn entry(&mut self) -> Result<ManifestEntry, CapsuleError> {
        if self.byte()? != crate::manifest::ENTRY_VERSION {
            return Err(CapsuleError::Malformed);
        }
        let info_hash = self.array()?;
        let name_len = self.u32()? as usize;
        let name = String::from_utf8(self.take(name_len)?.to_vec())
            .map_err(|_| CapsuleError::Malformed)?;
        let thumbnail_hash = match self.byte()? {
            0 => None,
            1 => Some(self.array()?),
            _ => return Err(CapsuleError::Malformed),
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
    use portalis_nexus_protocol::{DEVICE_KEY_BYTES, SIGNATURE_BYTES};

    use super::*;
    use crate::manifest::{INFO_HASH_BYTES, THUMBNAIL_HASH_BYTES};

    const KEY: ShareKey = [3; SHARE_KEY_BYTES];
    const SHARE: [u8; SHARE_ID_BYTES] = [5; SHARE_ID_BYTES];

    fn entry(info_hash: u8, name: &str, thumbnail: bool) -> ManifestEntry {
        ManifestEntry {
            info_hash: [info_hash; INFO_HASH_BYTES],
            name: name.to_owned(),
            thumbnail_hash: thumbnail.then_some([4; THUMBNAIL_HASH_BYTES]),
            author_public_key: [7; DEVICE_KEY_BYTES],
            added_at_unix_ns: 1_700_000_000_000_000_000,
            signature: [9; SIGNATURE_BYTES],
        }
    }

    fn manifest() -> Manifest {
        Manifest::new(vec![
            entry(1, "one.jpg", false),
            entry(2, "two.mp4", true),
            entry(3, "a name with spaces and é", false),
        ])
        .expect("built")
    }

    fn context(manifest: &Manifest, revision: u64) -> CapsuleContext {
        CapsuleContext {
            share_id: SHARE,
            revision,
            snapshot_id: manifest.snapshot_id(),
        }
    }

    #[test]
    fn a_sealed_manifest_opens_back_into_the_same_entries() {
        let manifest = manifest();
        let capsule = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        let opened = open(&KEY, &context(&manifest, 1), &capsule).expect("opened");

        assert_eq!(opened, manifest);
        assert_eq!(opened.snapshot_id(), manifest.snapshot_id());
        assert_eq!(capsule[0], CAPSULE_VERSION);
    }

    #[test]
    fn an_empty_manifest_round_trips_too() {
        let manifest = Manifest::default();
        let capsule = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        assert_eq!(
            open(&KEY, &context(&manifest, 1), &capsule).expect("opened"),
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

    /// The context is authenticated, so a capsule cannot be lifted from the
    /// share, revision, or snapshot it was sealed for onto another.
    #[test]
    fn a_capsule_does_not_open_anywhere_it_was_not_sealed() {
        let manifest = manifest();
        let capsule = seal(&KEY, SHARE, 1, &manifest).expect("sealed");
        let correct = context(&manifest, 1);

        for wrong in [
            CapsuleContext {
                share_id: [6; SHARE_ID_BYTES],
                ..correct
            },
            CapsuleContext {
                revision: 2,
                ..correct
            },
            CapsuleContext {
                snapshot_id: [0; 32],
                ..correct
            },
        ] {
            assert_eq!(
                open(&KEY, &wrong, &capsule),
                Err(CapsuleError::Rejected),
                "a capsule is bound to exactly one place"
            );
        }

        assert_eq!(
            open(&[4; SHARE_KEY_BYTES], &correct, &capsule),
            Err(CapsuleError::Rejected),
            "and to exactly one key"
        );
    }

    #[test]
    fn a_tampered_capsule_is_refused() {
        let manifest = manifest();
        let correct = context(&manifest, 1);
        let capsule = seal(&KEY, SHARE, 1, &manifest).expect("sealed");

        let mut flipped = capsule.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        assert_eq!(open(&KEY, &correct, &flipped), Err(CapsuleError::Rejected));

        let mut wrong_nonce = capsule.clone();
        wrong_nonce[1] ^= 0x01;
        assert_eq!(
            open(&KEY, &correct, &wrong_nonce),
            Err(CapsuleError::Rejected),
            "the nonce is derived, so a capsule carrying another is not ours"
        );

        assert_eq!(
            open(&KEY, &correct, &capsule[..capsule.len() - 1]),
            Err(CapsuleError::Rejected),
            "a truncated ciphertext fails its tag"
        );
    }

    #[test]
    fn a_capsule_too_short_or_too_new_is_refused_before_anything_else() {
        let manifest = manifest();
        let correct = context(&manifest, 1);

        assert_eq!(
            open(&KEY, &correct, &[]),
            Err(CapsuleError::TooShort { actual: 0 })
        );
        assert_eq!(
            open(&KEY, &correct, &[CAPSULE_VERSION, 1, 2]),
            Err(CapsuleError::TooShort { actual: 3 })
        );
        assert_eq!(
            open(&KEY, &correct, &[CAPSULE_VERSION + 1; 64]),
            Err(CapsuleError::UnsupportedVersion {
                actual: CAPSULE_VERSION + 1
            }),
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
                Err(CapsuleError::Malformed)
            );
        }
    }

    /// A capsule whose plaintext is a valid manifest, but not the one the
    /// context names, is refused: the root is what a revision points at.
    #[test]
    fn a_manifest_that_is_not_the_one_named_is_refused() {
        let manifest = manifest();
        let other = Manifest::new(vec![entry(9, "elsewhere", false)]).expect("built");

        let sealed = seal_raw(&KEY, &context(&manifest, 1), &other.encode());

        assert_eq!(
            open(&KEY, &context(&manifest, 1), &sealed),
            Err(CapsuleError::Malformed)
        );
    }

    /// Seals bytes that [`seal`] would never produce, so the reader can be
    /// tested against plaintext a hostile holder of the key could write.
    fn seal_raw(key: &ShareKey, context: &CapsuleContext, plaintext: &[u8]) -> Vec<u8> {
        let ciphertext = ChaCha20Poly1305::new(&Key::from(*key))
            .encrypt(
                &context.nonce(),
                Payload {
                    msg: plaintext,
                    aad: &context.associated_data(),
                },
            )
            .expect("test plaintext seals");
        let mut capsule = vec![CAPSULE_VERSION];
        capsule.extend_from_slice(context.nonce().as_slice());
        capsule.extend_from_slice(&ciphertext);
        capsule
    }
}
