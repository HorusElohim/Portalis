//! An entry's `.torrent`, encrypted.
//!
//! This is `SPEC.md` §7.5. The manifest already says what an entry is — its
//! label, author and info hash — so the payload carries nothing but the
//! descriptor itself. Everything the earlier encoding repeated here is read
//! from the manifest entry instead, which is also what stops the two
//! disagreeing.
//!
//! It travels separately from the manifest because a descriptor is up to
//! 4 MiB and most are never wanted immediately: a receiver fetches one when
//! it decides to download that entry.

use thiserror::Error;

use crate::format::aead::{self, AeadError, ContentKey, OVERHEAD_BYTES};
use crate::{INFO_HASH_V1_BYTES, MAX_SHARE_HANDOFF_BYTES, SHARE_ID_BYTES};

/// The encoding an entry payload declares in its first byte.
pub const ENTRY_PAYLOAD_VERSION: u8 = 1;

/// What an entry payload is bound to, and what fails to open it elsewhere.
///
/// The recipient device is deliberately absent. A payload is fetched by
/// whoever already holds the content key and is a member at that revision, so
/// binding it to one device would only stop a member reading it on their
/// second device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryContext {
    pub collection_id: [u8; SHARE_ID_BYTES],
    pub info_hash: [u8; INFO_HASH_V1_BYTES],
}

impl EntryContext {
    fn associated_data(&self) -> [u8; SHARE_ID_BYTES + INFO_HASH_V1_BYTES] {
        let mut data = [0_u8; SHARE_ID_BYTES + INFO_HASH_V1_BYTES];
        data[..SHARE_ID_BYTES].copy_from_slice(&self.collection_id);
        data[SHARE_ID_BYTES..].copy_from_slice(&self.info_hash);
        data
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EntryError {
    #[error("payload exceeds the {MAX_SHARE_HANDOFF_BYTES}-byte limit")]
    TooLarge,
    #[error("payload is empty")]
    Empty,
    #[error(transparent)]
    Sealed(#[from] AeadError),
}

/// Encrypts one entry's descriptor under the collection's content key.
///
/// The nonce is drawn fresh for every call rather than derived: unlike a
/// manifest, a payload has no revision to make a derived nonce unique, and
/// re-sending the same descriptor is not something a receiver needs to
/// recognise byte for byte.
///
/// # Errors
///
/// Returns [`EntryError`] when the plaintext is empty or the result would
/// exceed the protocol limit.
pub fn seal(
    key: &ContentKey,
    context: &EntryContext,
    plaintext: &[u8],
) -> Result<Vec<u8>, EntryError> {
    if plaintext.is_empty() {
        return Err(EntryError::Empty);
    }
    if plaintext.len() + OVERHEAD_BYTES > MAX_SHARE_HANDOFF_BYTES {
        return Err(EntryError::TooLarge);
    }
    Ok(aead::seal(
        key,
        ENTRY_PAYLOAD_VERSION,
        aead::random_nonce(),
        &context.associated_data(),
        plaintext,
    ))
}

/// Opens an entry payload and returns the descriptor inside it.
///
/// The caller still validates that descriptor — that it is private, and that
/// its computed info hash matches the entry — because opening proves only who
/// encrypted it, not what they encrypted.
///
/// # Errors
///
/// Returns [`EntryError`] when the payload is oversized, or [`AeadError`]
/// through it when the value is truncated, versioned differently, or does not
/// open under this key and context.
pub fn open(
    key: &ContentKey,
    context: &EntryContext,
    sealed: &[u8],
) -> Result<Vec<u8>, EntryError> {
    if sealed.len() > MAX_SHARE_HANDOFF_BYTES {
        return Err(EntryError::TooLarge);
    }
    Ok(aead::open(
        key,
        ENTRY_PAYLOAD_VERSION,
        &context.associated_data(),
        sealed,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: ContentKey = [3; 32];
    const OTHER_KEY: ContentKey = [4; 32];
    const TORRENT: &[u8] = b"d8:announce0:4:infod6:lengthi1e4:name1:a12:piece lengthi1eee";

    fn context() -> EntryContext {
        EntryContext {
            collection_id: [5; SHARE_ID_BYTES],
            info_hash: [6; INFO_HASH_V1_BYTES],
        }
    }

    #[test]
    fn a_descriptor_round_trips() {
        let sealed = seal(&KEY, &context(), TORRENT).expect("sealed");

        assert_eq!(sealed[0], ENTRY_PAYLOAD_VERSION);
        assert_eq!(open(&KEY, &context(), &sealed).expect("opened"), TORRENT);
    }

    /// A fresh nonce per call, so two seals of one descriptor differ — and
    /// both still open, which is what says the nonce travels with the bytes.
    #[test]
    fn sealing_twice_differs_and_both_open() {
        let first = seal(&KEY, &context(), TORRENT).expect("sealed");
        let second = seal(&KEY, &context(), TORRENT).expect("sealed");

        assert_ne!(first, second);
        assert_eq!(open(&KEY, &context(), &first).expect("opened"), TORRENT);
        assert_eq!(open(&KEY, &context(), &second).expect("opened"), TORRENT);
    }

    #[test]
    fn a_payload_does_not_open_anywhere_it_was_not_sealed() {
        let sealed = seal(&KEY, &context(), TORRENT).expect("sealed");

        for wrong in [
            EntryContext {
                collection_id: [9; SHARE_ID_BYTES],
                ..context()
            },
            EntryContext {
                info_hash: [9; INFO_HASH_V1_BYTES],
                ..context()
            },
        ] {
            assert_eq!(
                open(&KEY, &wrong, &sealed),
                Err(EntryError::Sealed(AeadError::Rejected))
            );
        }
        assert_eq!(
            open(&OTHER_KEY, &context(), &sealed),
            Err(EntryError::Sealed(AeadError::Rejected)),
            "and not under another key"
        );
    }

    #[test]
    fn a_tampered_or_truncated_payload_is_refused() {
        let sealed = seal(&KEY, &context(), TORRENT).expect("sealed");

        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        assert_eq!(
            open(&KEY, &context(), &flipped),
            Err(EntryError::Sealed(AeadError::Rejected))
        );

        assert_eq!(
            open(&KEY, &context(), &sealed[..sealed.len() - 1]),
            Err(EntryError::Sealed(AeadError::Rejected))
        );
    }

    #[test]
    fn a_payload_too_short_or_too_new_is_refused_before_anything_else() {
        assert_eq!(
            open(&KEY, &context(), &[]),
            Err(EntryError::Sealed(AeadError::TooShort { actual: 0 }))
        );
        assert_eq!(
            open(&KEY, &context(), &[ENTRY_PAYLOAD_VERSION, 1, 2]),
            Err(EntryError::Sealed(AeadError::TooShort { actual: 3 }))
        );
        assert_eq!(
            open(&KEY, &context(), &[ENTRY_PAYLOAD_VERSION + 1; 64]),
            Err(EntryError::Sealed(AeadError::UnsupportedVersion {
                expected: ENTRY_PAYLOAD_VERSION,
                actual: ENTRY_PAYLOAD_VERSION + 1
            }))
        );
    }

    #[test]
    fn the_limits_are_enforced_at_both_ends() {
        assert_eq!(seal(&KEY, &context(), &[]), Err(EntryError::Empty));
        assert_eq!(
            seal(&KEY, &context(), &vec![0; MAX_SHARE_HANDOFF_BYTES]),
            Err(EntryError::TooLarge)
        );
        assert_eq!(
            open(&KEY, &context(), &vec![0; MAX_SHARE_HANDOFF_BYTES + 1]),
            Err(EntryError::TooLarge)
        );

        // One byte under the limit still seals, so the boundary is exact.
        let largest = MAX_SHARE_HANDOFF_BYTES - OVERHEAD_BYTES;
        assert!(seal(&KEY, &context(), &vec![0; largest]).is_ok());
    }
}
