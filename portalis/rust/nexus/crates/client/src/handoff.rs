//! Versioned encrypted torrent handoffs between authorized devices.
//!
//! Nexus forwards the resulting bytes but never sees the collection name,
//! torrent descriptor, or share key. The wrapper carries its own version and
//! random nonce; the routing envelope carries the info hash so the receiver
//! can construct the authenticated context before opening it.

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use portalis_nexus_protocol::{
    DEVICE_ID_BYTES, INFO_HASH_V1_BYTES, MAX_SHARE_HANDOFF_BYTES, SHARE_ID_BYTES, new_challenge,
};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use crate::capsule::ShareKey;

/// The handoff encoding carried inside the protocol's `ShareHandoff` message.
pub const HANDOFF_VERSION: u8 = 1;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const MAX_COLLECTION_NAME_BYTES: usize = 1_024;
const FIXED_PLAINTEXT_BYTES: usize = INFO_HASH_V1_BYTES + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandoffContext {
    pub share_id: [u8; SHARE_ID_BYTES],
    pub recipient_device_id: [u8; DEVICE_ID_BYTES],
    pub info_hash: [u8; INFO_HASH_V1_BYTES],
}

impl HandoffContext {
    fn associated_data(&self) -> [u8; SHARE_ID_BYTES + DEVICE_ID_BYTES + INFO_HASH_V1_BYTES] {
        let mut data = [0_u8; SHARE_ID_BYTES + DEVICE_ID_BYTES + INFO_HASH_V1_BYTES];
        let share_end = SHARE_ID_BYTES;
        let device_end = share_end + DEVICE_ID_BYTES;
        data[..share_end].copy_from_slice(&self.share_id);
        data[share_end..device_end].copy_from_slice(&self.recipient_device_id);
        data[device_end..].copy_from_slice(&self.info_hash);
        data
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentHandoff {
    pub collection_name: String,
    pub info_hash: [u8; INFO_HASH_V1_BYTES],
    pub torrent_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HandoffError {
    #[error("handoff is {actual} bytes, too short to hold a version and nonce")]
    TooShort { actual: usize },
    #[error("handoff declares version {actual}, and this client speaks {HANDOFF_VERSION}")]
    UnsupportedVersion { actual: u8 },
    #[error("handoff ciphertext exceeds the {MAX_SHARE_HANDOFF_BYTES}-byte limit")]
    TooLarge,
    #[error("handoff did not open")]
    Rejected,
    #[error("handoff plaintext is malformed")]
    Malformed,
    #[error("collection name is not NFC-normalized")]
    NameNotNfc,
    #[error("collection name is {actual} bytes, over the {MAX_COLLECTION_NAME_BYTES}-byte limit")]
    NameTooLong { actual: usize },
    #[error("torrent descriptor is empty")]
    EmptyTorrent,
}

/// Encrypts one collection entry for one recipient device.
///
/// # Errors
///
/// Returns [`HandoffError`] when the name or descriptor violates the format
/// limits, the name is not NFC-normalized, or encryption fails.
pub fn seal_handoff(
    key: &ShareKey,
    context: &HandoffContext,
    collection_name: &str,
    torrent_bytes: &[u8],
) -> Result<Vec<u8>, HandoffError> {
    validate_name(collection_name)?;
    if torrent_bytes.is_empty() {
        return Err(HandoffError::EmptyTorrent);
    }

    let name = collection_name.as_bytes();
    let plaintext_len = 4usize
        .checked_add(name.len())
        .and_then(|length| length.checked_add(FIXED_PLAINTEXT_BYTES))
        .and_then(|length| length.checked_add(torrent_bytes.len()))
        .ok_or(HandoffError::TooLarge)?;
    let encoded_len = 1usize
        .checked_add(NONCE_BYTES)
        .and_then(|length| length.checked_add(plaintext_len))
        .and_then(|length| length.checked_add(TAG_BYTES))
        .ok_or(HandoffError::TooLarge)?;
    if encoded_len > MAX_SHARE_HANDOFF_BYTES {
        return Err(HandoffError::TooLarge);
    }

    let mut plaintext = Vec::with_capacity(plaintext_len);
    plaintext.extend_from_slice(
        &u32::try_from(name.len())
            .map_err(|_| HandoffError::TooLarge)?
            .to_le_bytes(),
    );
    plaintext.extend_from_slice(name);
    plaintext.extend_from_slice(&context.info_hash);
    plaintext.extend_from_slice(
        &u32::try_from(torrent_bytes.len())
            .map_err(|_| HandoffError::TooLarge)?
            .to_le_bytes(),
    );
    plaintext.extend_from_slice(torrent_bytes);

    let random = new_challenge();
    let mut nonce_bytes = [0_u8; NONCE_BYTES];
    nonce_bytes.copy_from_slice(&random[..NONCE_BYTES]);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = ChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(
            &nonce,
            Payload {
                msg: &plaintext,
                aad: &context.associated_data(),
            },
        )
        .map_err(|_| HandoffError::Rejected)?;

    let mut encoded = Vec::with_capacity(encoded_len);
    encoded.push(HANDOFF_VERSION);
    encoded.extend_from_slice(&nonce_bytes);
    encoded.extend_from_slice(&ciphertext);
    Ok(encoded)
}

/// Opens a handoff after routing has authenticated its share, device, and info hash.
///
/// # Errors
///
/// Returns [`HandoffError`] when the handoff is oversized, truncated,
/// unsupported, malformed, tampered with, or bound to a different context.
pub fn open_handoff(
    key: &ShareKey,
    context: &HandoffContext,
    encoded: &[u8],
) -> Result<TorrentHandoff, HandoffError> {
    if encoded.len() > MAX_SHARE_HANDOFF_BYTES {
        return Err(HandoffError::TooLarge);
    }
    let (&version, rest) = encoded.split_first().ok_or(HandoffError::TooShort {
        actual: encoded.len(),
    })?;
    if version != HANDOFF_VERSION {
        return Err(HandoffError::UnsupportedVersion { actual: version });
    }
    if rest.len() < NONCE_BYTES + TAG_BYTES {
        return Err(HandoffError::TooShort {
            actual: encoded.len(),
        });
    }
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_BYTES);
    let nonce = Nonce::from(
        <[u8; NONCE_BYTES]>::try_from(nonce_bytes).map_err(|_| HandoffError::Malformed)?,
    );
    let plaintext = ChaCha20Poly1305::new(&Key::from(*key))
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: &context.associated_data(),
            },
        )
        .map_err(|_| HandoffError::Rejected)?;
    decode_plaintext(context, &plaintext)
}

fn validate_name(name: &str) -> Result<(), HandoffError> {
    if name.len() > MAX_COLLECTION_NAME_BYTES {
        return Err(HandoffError::NameTooLong { actual: name.len() });
    }
    if name.nfc().collect::<String>() != name {
        return Err(HandoffError::NameNotNfc);
    }
    Ok(())
}

fn decode_plaintext(
    context: &HandoffContext,
    plaintext: &[u8],
) -> Result<TorrentHandoff, HandoffError> {
    let mut reader = Reader::new(plaintext);
    let name_len = reader.u32()? as usize;
    if name_len > MAX_COLLECTION_NAME_BYTES {
        return Err(HandoffError::NameTooLong { actual: name_len });
    }
    let collection_name =
        String::from_utf8(reader.take(name_len)?.to_vec()).map_err(|_| HandoffError::Malformed)?;
    validate_name(&collection_name)?;
    let info_hash = reader.array::<INFO_HASH_V1_BYTES>()?;
    if info_hash != context.info_hash {
        return Err(HandoffError::Rejected);
    }
    let torrent_len = reader.u32()? as usize;
    if torrent_len == 0 {
        return Err(HandoffError::EmptyTorrent);
    }
    let torrent_bytes = reader.take(torrent_len)?.to_vec();
    if !reader.is_empty() {
        return Err(HandoffError::Malformed);
    }
    Ok(TorrentHandoff {
        collection_name,
        info_hash,
        torrent_bytes,
    })
}

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

    fn take(&mut self, count: usize) -> Result<&'a [u8], HandoffError> {
        if self.bytes.len() < count {
            return Err(HandoffError::Malformed);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], HandoffError> {
        self.take(N)
            .and_then(|bytes| <[u8; N]>::try_from(bytes).map_err(|_| HandoffError::Malformed))
    }

    fn u32(&mut self) -> Result<u32, HandoffError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: ShareKey = [7; 32];
    const CONTEXT: HandoffContext = HandoffContext {
        share_id: [1; SHARE_ID_BYTES],
        recipient_device_id: [2; DEVICE_ID_BYTES],
        info_hash: [3; INFO_HASH_V1_BYTES],
    };

    #[test]
    fn a_handoff_round_trips() {
        let encoded = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        let opened = open_handoff(&KEY, &CONTEXT, &encoded).unwrap();
        assert_eq!(opened.collection_name, "Family");
        assert_eq!(opened.info_hash, CONTEXT.info_hash);
        assert_eq!(opened.torrent_bytes, b"torrent bytes");
    }

    #[test]
    fn each_attempt_gets_a_fresh_nonce() {
        let first = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        let second = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn changing_context_rejects_the_handoff() {
        let encoded = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        let mut other = CONTEXT;
        other.recipient_device_id[0] ^= 1;
        assert_eq!(
            open_handoff(&KEY, &other, &encoded),
            Err(HandoffError::Rejected)
        );
    }

    #[test]
    fn a_context_info_hash_mismatch_is_rejected() {
        let mut other = CONTEXT;
        other.info_hash[0] ^= 1;
        let encoded = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        assert_eq!(
            open_handoff(&KEY, &other, &encoded),
            Err(HandoffError::Rejected)
        );
    }

    #[test]
    fn malformed_names_and_empty_torrents_are_refused() {
        assert_eq!(
            seal_handoff(&KEY, &CONTEXT, "e\u{301}", b"torrent"),
            Err(HandoffError::NameNotNfc)
        );
        assert_eq!(
            seal_handoff(&KEY, &CONTEXT, "Family", b""),
            Err(HandoffError::EmptyTorrent)
        );
    }

    #[test]
    fn tampering_and_truncation_are_refused() {
        let encoded = seal_handoff(&KEY, &CONTEXT, "Family", b"torrent bytes").unwrap();
        let mut tampered = encoded.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            open_handoff(&KEY, &CONTEXT, &tampered),
            Err(HandoffError::Rejected)
        );
        assert!(matches!(
            open_handoff(&KEY, &CONTEXT, &encoded[..NONCE_BYTES]),
            Err(HandoffError::TooShort { .. })
        ));
    }
}
