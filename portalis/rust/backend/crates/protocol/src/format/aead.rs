//! The one place bytes are encrypted.
//!
//! Every sealed thing in Portalis has the same envelope — a version byte, a
//! nonce, and `ChaCha20-Poly1305` ciphertext with its tag — and differs only
//! in what it authenticates alongside and how it chooses the nonce. Those two
//! choices are the contract; the framing is not, so it lives here once.
//!
//! Nothing in this module knows what it is encrypting. A caller supplies a
//! key, a version, associated data and bytes, which is why the same code
//! serves a manifest, a torrent descriptor, and whatever comes next.
//!
//! ```text
//! sealed := u8      version
//!           u8[12]  nonce
//!           u8[]    ciphertext        tag included
//! ```

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use thiserror::Error;

use crate::new_challenge;

/// A symmetric key. One per collection, never seen by the service.
pub const CONTENT_KEY_BYTES: usize = 32;
pub type ContentKey = [u8; CONTENT_KEY_BYTES];

pub const NONCE_BYTES: usize = 12;
/// `ChaCha20-Poly1305`'s authentication tag, included in the ciphertext.
pub const TAG_BYTES: usize = 16;
/// The smallest possible sealed value: version, nonce, tag, no plaintext.
pub const OVERHEAD_BYTES: usize = 1 + NONCE_BYTES + TAG_BYTES;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AeadError {
    #[error("sealed value is {actual} bytes, too short to hold a version, nonce and tag")]
    TooShort { actual: usize },
    #[error("sealed value declares version {actual}, and this reader speaks {expected}")]
    UnsupportedVersion { expected: u8, actual: u8 },
    /// Wrong key, wrong context, wrong nonce, or tampered bytes — one answer,
    /// because telling them apart would say more about the ciphertext than a
    /// failed open should.
    #[error("the sealed value did not open")]
    Rejected,
}

/// A nonce drawn from the operating system, for values with nothing unique to
/// derive from.
#[must_use]
pub fn random_nonce() -> [u8; NONCE_BYTES] {
    let mut nonce = [0_u8; NONCE_BYTES];
    nonce.copy_from_slice(&new_challenge()[..NONCE_BYTES]);
    nonce
}

/// A nonce derived from values that are unique per sealing under one key.
///
/// Deterministic on purpose: re-sealing the same thing produces identical
/// bytes, which is what lets a lost acknowledgement be retried without the
/// receiver seeing a second, different object.
#[must_use]
pub fn derived_nonce(domain: &[u8], parts: &[&[u8]]) -> [u8; NONCE_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    let mut nonce = [0_u8; NONCE_BYTES];
    nonce.copy_from_slice(&hasher.finalize().as_bytes()[..NONCE_BYTES]);
    nonce
}

/// Seals `plaintext`, authenticating `aad` alongside it.
///
/// # Errors
///
/// Returns [`AeadError::Rejected`] only for a plaintext far larger than
/// anything this protocol carries; the limit is the caller's to enforce.
pub fn seal(
    key: &ContentKey,
    version: u8,
    nonce: [u8; NONCE_BYTES],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let ciphertext = ChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AeadError::Rejected)?;

    let mut sealed = Vec::with_capacity(1 + NONCE_BYTES + ciphertext.len());
    sealed.push(version);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// Opens a sealed value, accepting whatever nonce it carries.
///
/// # Errors
///
/// Returns [`AeadError`] when the value is truncated, declares another
/// version, or does not open under this key and associated data.
pub fn open(
    key: &ContentKey,
    version: u8,
    aad: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let (nonce, ciphertext) = split(version, sealed)?;
    ChaCha20Poly1305::new(&Key::from(*key))
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| AeadError::Rejected)
}

/// Opens a sealed value whose nonce the reader can derive for itself.
///
/// A carried nonce that differs from the expected one means the value was not
/// sealed for this position, so it is refused before any decryption is
/// attempted.
///
/// # Errors
///
/// Returns [`AeadError`] as [`open`] does, and [`AeadError::Rejected`] when
/// the carried nonce is not the expected one.
pub fn open_derived(
    key: &ContentKey,
    version: u8,
    expected_nonce: [u8; NONCE_BYTES],
    aad: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let (nonce, _) = split(version, sealed)?;
    if nonce != expected_nonce {
        return Err(AeadError::Rejected);
    }
    open(key, version, aad, sealed)
}

fn split(version: u8, sealed: &[u8]) -> Result<([u8; NONCE_BYTES], &[u8]), AeadError> {
    let (&declared, rest) = sealed.split_first().ok_or(AeadError::TooShort {
        actual: sealed.len(),
    })?;
    if declared != version {
        return Err(AeadError::UnsupportedVersion {
            expected: version,
            actual: declared,
        });
    }
    if rest.len() < NONCE_BYTES + TAG_BYTES {
        return Err(AeadError::TooShort {
            actual: sealed.len(),
        });
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_BYTES);
    let nonce = <[u8; NONCE_BYTES]>::try_from(nonce).map_err(|_| AeadError::Rejected)?;
    Ok((nonce, ciphertext))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: ContentKey = [3; CONTENT_KEY_BYTES];
    const VERSION: u8 = 1;

    #[test]
    fn a_value_round_trips_under_its_own_key_and_context() {
        let sealed = seal(&KEY, VERSION, random_nonce(), b"aad", b"hello").expect("sealed");

        assert_eq!(sealed[0], VERSION);
        assert_eq!(
            open(&KEY, VERSION, b"aad", &sealed).expect("opened"),
            b"hello"
        );
        assert_eq!(
            open(&KEY, VERSION, b"other", &sealed),
            Err(AeadError::Rejected),
            "associated data is authenticated"
        );
        assert_eq!(
            open(&[4; CONTENT_KEY_BYTES], VERSION, b"aad", &sealed),
            Err(AeadError::Rejected)
        );
    }

    /// The property the manifest depends on: sealing the same thing twice
    /// gives identical bytes, so a retry is recognisable.
    #[test]
    fn a_derived_nonce_is_stable_and_a_random_one_is_not() {
        let derived = derived_nonce(b"domain", &[b"a", b"b"]);

        assert_eq!(derived, derived_nonce(b"domain", &[b"a", b"b"]));
        assert_ne!(derived, derived_nonce(b"domain", &[b"a", b"c"]));
        assert_ne!(derived, derived_nonce(b"other", &[b"a", b"b"]));
        assert_ne!(random_nonce(), random_nonce());
    }

    #[test]
    fn a_derived_open_refuses_a_nonce_it_did_not_expect() {
        let expected = derived_nonce(b"domain", &[b"one"]);
        let sealed = seal(&KEY, VERSION, expected, b"aad", b"hello").expect("sealed");

        assert_eq!(
            open_derived(&KEY, VERSION, expected, b"aad", &sealed).expect("opened"),
            b"hello"
        );
        assert_eq!(
            open_derived(
                &KEY,
                VERSION,
                derived_nonce(b"domain", &[b"two"]),
                b"aad",
                &sealed
            ),
            Err(AeadError::Rejected),
            "a value sealed for another position is refused before decryption"
        );
    }

    #[test]
    fn truncated_tampered_and_unknown_versions_are_refused() {
        let sealed = seal(&KEY, VERSION, random_nonce(), b"aad", b"hello").expect("sealed");

        assert_eq!(
            open(&KEY, VERSION, b"aad", &[]),
            Err(AeadError::TooShort { actual: 0 })
        );
        assert_eq!(
            open(&KEY, VERSION, b"aad", &sealed[..OVERHEAD_BYTES - 1]),
            Err(AeadError::TooShort {
                actual: OVERHEAD_BYTES - 1
            })
        );
        assert_eq!(
            open(&KEY, VERSION, b"aad", &[VERSION + 1; 64]),
            Err(AeadError::UnsupportedVersion {
                expected: VERSION,
                actual: VERSION + 1
            })
        );

        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        assert_eq!(
            open(&KEY, VERSION, b"aad", &flipped),
            Err(AeadError::Rejected)
        );
    }

    /// An empty plaintext still seals, because emptiness is the caller's rule
    /// to make, not this layer's.
    #[test]
    fn an_empty_plaintext_is_this_layers_business_to_carry_not_to_judge() {
        let sealed = seal(&KEY, VERSION, random_nonce(), b"", b"").expect("sealed");

        assert_eq!(sealed.len(), OVERHEAD_BYTES);
        assert_eq!(open(&KEY, VERSION, b"", &sealed).expect("opened"), b"");
    }
}
