//! Sealing a short secret to one recipient's X25519 public key.
//!
//! Sealing needs only the recipient's public key: whoever knows it can
//! produce an envelope, which is what lets one device push a share key to
//! another it has never talked to directly. Opening needs the recipient's
//! private key, which this crate never holds — `open` is a pure function a
//! caller runs with a secret it keeps itself, the same split `DeviceSigner`
//! draws between a device's public key and whatever holds its private half.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::SimpleHkdf;
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

use crate::limits::{DEVICE_ID_BYTES, ENCRYPTION_KEY_BYTES, SHARE_ID_BYTES};

/// Domain separator mixed into every derived key, so a key agreed for this
/// operation can never be reinterpreted as one from another.
const KEY_CONTEXT: &[u8] = b"portalis.protocol.v1/key-envelope/key";
const ASSOCIATED_DATA_CONTEXT: &[u8] = b"portalis.protocol.v1/key-envelope/aad";
const ENVELOPE_VERSION: u8 = 1;

/// The `ChaCha20`-Poly1305 nonce every envelope is sealed under.
///
/// Safe to fix rather than randomize: the symmetric key is derived fresh from
/// a one-time ephemeral Diffie-Hellman exchange for every single seal, so the
/// (key, nonce) pair this nonce contributes to is never reused.
const NONCE: [u8; 12] = [0; 12];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SealError {
    #[error("public key must contain exactly {ENCRYPTION_KEY_BYTES} bytes, got {actual}")]
    InvalidPublicKeyLength { actual: usize },
    #[error("secret key must contain exactly {ENCRYPTION_KEY_BYTES} bytes, got {actual}")]
    InvalidSecretKeyLength { actual: usize },
    #[error("X25519 public key is non-contributory")]
    NonContributoryPublicKey,
    #[error("the envelope does not open with this key")]
    Rejected,
}

/// A secret, sealed so only the holder of one recipient's private key can
/// recover it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedEnvelope {
    /// The one-time public key `open` needs to complete the key exchange.
    pub ephemeral_public_key: [u8; ENCRYPTION_KEY_BYTES],
    pub ciphertext: Vec<u8>,
}

/// Immutable facts authenticated alongside one sealed share key.
///
/// The server stores these fields next to the ciphertext. Authenticating them
/// makes a copied database row fail to open under a different share or device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeContext {
    pub share_id: [u8; SHARE_ID_BYTES],
    pub recipient_device_id: [u8; DEVICE_ID_BYTES],
}

/// Seals `plaintext` to `recipient_public_key`, so only whoever holds the
/// matching secret can recover it.
///
/// # Errors
///
/// Returns [`SealError`] when the recipient's public key has the wrong
/// length.
///
/// # Panics
///
/// Never in practice: `ChaCha20Poly1305` only refuses to seal a plaintext
/// longer than roughly 256 GiB, far past anything a key envelope carries.
pub fn seal(
    recipient_public_key: &[u8],
    context: &EnvelopeContext,
    plaintext: &[u8],
) -> Result<SealedEnvelope, SealError> {
    let recipient = parse_public_key(recipient_public_key)?;
    let ephemeral_secret = EphemeralSecret::random();
    let ephemeral_public_key = PublicKey::from(&ephemeral_secret);
    let shared = ephemeral_secret.diffie_hellman(&recipient);

    let key = derive_key(
        shared.as_bytes(),
        ephemeral_public_key.as_bytes(),
        recipient.as_bytes(),
    );
    let associated_data = associated_data(context);
    let ciphertext = ChaCha20Poly1305::new(&Key::from(key))
        .encrypt(
            &Nonce::from(NONCE),
            Payload {
                msg: plaintext,
                aad: &associated_data,
            },
        )
        .expect("a fresh key and fixed nonce always seal");

    Ok(SealedEnvelope {
        ephemeral_public_key: *ephemeral_public_key.as_bytes(),
        ciphertext,
    })
}

/// Opens an envelope [`seal`] produced for the holder of `recipient_secret_key`.
///
/// # Errors
///
/// Returns [`SealError`] when the secret key has the wrong length or the
/// envelope was not sealed to it.
pub fn open(
    recipient_secret_key: &[u8],
    context: &EnvelopeContext,
    envelope: &SealedEnvelope,
) -> Result<Vec<u8>, SealError> {
    let secret_bytes: [u8; ENCRYPTION_KEY_BYTES] =
        recipient_secret_key
            .try_into()
            .map_err(|_| SealError::InvalidSecretKeyLength {
                actual: recipient_secret_key.len(),
            })?;
    let secret = StaticSecret::from(secret_bytes);
    let recipient_public_key = PublicKey::from(&secret);
    let ephemeral_public_key = PublicKey::from(envelope.ephemeral_public_key);
    let shared = secret.diffie_hellman(&ephemeral_public_key);
    if !shared.was_contributory() {
        return Err(SealError::Rejected);
    }

    let key = derive_key(
        shared.as_bytes(),
        &envelope.ephemeral_public_key,
        recipient_public_key.as_bytes(),
    );
    let associated_data = associated_data(context);
    ChaCha20Poly1305::new(&Key::from(key))
        .decrypt(
            &Nonce::from(NONCE),
            Payload {
                msg: envelope.ciphertext.as_slice(),
                aad: &associated_data,
            },
        )
        .map_err(|_| SealError::Rejected)
}

fn parse_public_key(bytes: &[u8]) -> Result<PublicKey, SealError> {
    let fixed: [u8; ENCRYPTION_KEY_BYTES] =
        bytes
            .try_into()
            .map_err(|_| SealError::InvalidPublicKeyLength {
                actual: bytes.len(),
            })?;
    if !is_contributory_x25519_public_key(&fixed) {
        return Err(SealError::NonContributoryPublicKey);
    }
    Ok(PublicKey::from(fixed))
}

/// Whether an X25519 public key can contribute a secret to a key exchange.
///
/// Low-order public keys produce an all-zero shared secret for every private
/// key. They must never be registered or used to seal an envelope.
#[must_use]
pub fn is_contributory_x25519_public_key(public_key: &[u8; ENCRYPTION_KEY_BYTES]) -> bool {
    let validation_secret = StaticSecret::from([0xA5; ENCRYPTION_KEY_BYTES]);
    validation_secret
        .diffie_hellman(&PublicKey::from(*public_key))
        .was_contributory()
}

/// Binds the agreed secret to both public keys, so a key derived for one
/// (ephemeral, recipient) pair can never be reused for another.
fn derive_key(
    shared_secret: &[u8; 32],
    ephemeral_public_key: &[u8; 32],
    recipient_public_key: &[u8; 32],
) -> [u8; 32] {
    let hkdf = SimpleHkdf::<Sha256>::new(None, shared_secret);
    let mut info = Vec::with_capacity(KEY_CONTEXT.len() + 64);
    info.extend_from_slice(KEY_CONTEXT);
    info.extend_from_slice(ephemeral_public_key);
    info.extend_from_slice(recipient_public_key);
    let mut key = [0_u8; 32];
    hkdf.expand(&info, &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

fn associated_data(context: &EnvelopeContext) -> Vec<u8> {
    let mut data =
        Vec::with_capacity(ASSOCIATED_DATA_CONTEXT.len() + 1 + SHARE_ID_BYTES + DEVICE_ID_BYTES);
    data.extend_from_slice(ASSOCIATED_DATA_CONTEXT);
    data.push(ENVELOPE_VERSION);
    data.extend_from_slice(&context.share_id);
    data.extend_from_slice(&context.recipient_device_id);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair(seed: u8) -> (StaticSecret, PublicKey) {
        let secret = StaticSecret::from([seed; 32]);
        let public = PublicKey::from(&secret);
        (secret, public)
    }

    fn context() -> EnvelopeContext {
        EnvelopeContext {
            share_id: [3; SHARE_ID_BYTES],
            recipient_device_id: [4; DEVICE_ID_BYTES],
        }
    }

    #[test]
    fn a_sealed_envelope_opens_with_the_matching_secret() {
        let (secret, public) = keypair(7);

        let context = context();
        let envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");
        let opened = open(&secret.to_bytes(), &context, &envelope).expect("opens");

        assert_eq!(opened, b"share key material");
    }

    #[test]
    fn sealing_the_same_plaintext_twice_produces_different_envelopes() {
        let (_, public) = keypair(7);

        let context = context();
        let first = seal(public.as_bytes(), &context, b"share key material").expect("seals");
        let second = seal(public.as_bytes(), &context, b"share key material").expect("seals");

        // A fresh ephemeral key every time, so nothing about the ciphertext
        // or the exchange repeats.
        assert_ne!(first.ephemeral_public_key, second.ephemeral_public_key);
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn a_wrong_secret_cannot_open_the_envelope() {
        let (_, public) = keypair(7);
        let (stranger, _) = keypair(9);

        let context = context();
        let envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");

        assert_eq!(
            open(&stranger.to_bytes(), &context, &envelope),
            Err(SealError::Rejected)
        );
    }

    #[test]
    fn a_tampered_ciphertext_is_rejected() {
        let (secret, public) = keypair(7);
        let context = context();
        let mut envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");
        *envelope.ciphertext.last_mut().expect("nonempty") ^= 0xFF;

        assert_eq!(
            open(&secret.to_bytes(), &context, &envelope),
            Err(SealError::Rejected)
        );
    }

    #[test]
    fn a_tampered_ephemeral_key_is_rejected() {
        let (secret, public) = keypair(7);
        let context = context();
        let mut envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");
        envelope.ephemeral_public_key[0] ^= 0xFF;

        assert_eq!(
            open(&secret.to_bytes(), &context, &envelope),
            Err(SealError::Rejected)
        );
    }

    #[test]
    fn rejects_a_malformed_recipient_public_key() {
        assert_eq!(
            seal(&[0; 10], &context(), b"plaintext"),
            Err(SealError::InvalidPublicKeyLength { actual: 10 })
        );
    }

    #[test]
    fn rejects_a_malformed_secret_key() {
        let (_, public) = keypair(7);
        let context = context();
        let envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");

        assert_eq!(
            open(&[0; 10], &context, &envelope),
            Err(SealError::InvalidSecretKeyLength { actual: 10 })
        );
    }

    #[test]
    fn rejects_a_non_contributory_public_key() {
        assert_eq!(
            seal(&[0; ENCRYPTION_KEY_BYTES], &context(), b"plaintext"),
            Err(SealError::NonContributoryPublicKey)
        );
    }

    /// The same check on the way back in. An envelope naming a low-order
    /// ephemeral key would agree a shared secret of all zeros with anyone, so
    /// opening it is refused before the ciphertext is touched — as a failure
    /// to open, telling a caller nothing about why.
    #[test]
    fn an_envelope_naming_a_non_contributory_ephemeral_key_is_refused() {
        let (secret, public) = keypair(7);
        let context = context();
        let envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");

        let forged = SealedEnvelope {
            ephemeral_public_key: [0; ENCRYPTION_KEY_BYTES],
            ..envelope
        };

        assert_eq!(
            open(secret.as_bytes(), &context, &forged),
            Err(SealError::Rejected)
        );
    }

    #[test]
    fn an_envelope_cannot_be_transplanted_to_another_share_or_device() {
        let (secret, public) = keypair(7);
        let context = context();
        let envelope = seal(public.as_bytes(), &context, b"share key material").expect("seals");
        let other_share = EnvelopeContext {
            share_id: [9; SHARE_ID_BYTES],
            ..context
        };
        let other_device = EnvelopeContext {
            recipient_device_id: [8; DEVICE_ID_BYTES],
            ..context
        };

        assert_eq!(
            open(&secret.to_bytes(), &other_share, &envelope),
            Err(SealError::Rejected)
        );
        assert_eq!(
            open(&secret.to_bytes(), &other_device, &envelope),
            Err(SealError::Rejected)
        );
    }
}
