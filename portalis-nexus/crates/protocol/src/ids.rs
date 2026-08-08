//! Binary identifier construction and log-safe formatting.

use uuid::Uuid;

use crate::limits::{CHALLENGE_BYTES, DEVICE_ID_BYTES};

/// Domain separator for device identifiers, so the digest of a public key can
/// never collide with a digest computed for any other purpose.
const DEVICE_ID_CONTEXT: &str = "portalis.protocol.v1 device-id";

/// Derives a device's stable identifier from its Ed25519 public key.
///
/// Derivation is deterministic, so a device that already has a Portalis
/// keypair keeps the same Nexus identity without storing anything new.
#[must_use]
pub fn derive_device_id(device_public_key: &[u8]) -> [u8; DEVICE_ID_BYTES] {
    let mut hasher = blake3::Hasher::new_derive_key(DEVICE_ID_CONTEXT);
    hasher.update(device_public_key);
    *hasher.finalize().as_bytes()
}

#[must_use]
pub fn new_message_id() -> Vec<u8> {
    Uuid::now_v7().as_bytes().to_vec()
}

#[must_use]
pub fn new_challenge() -> Vec<u8> {
    let mut challenge = Vec::with_capacity(CHALLENGE_BYTES);
    challenge.extend_from_slice(Uuid::new_v4().as_bytes());
    challenge.extend_from_slice(Uuid::new_v4().as_bytes());
    challenge
}

/// Formats a 16-byte identifier for logs and spans.
///
/// Returns `unknown` for malformed identifiers so tracing never panics on
/// attacker-controlled input.
#[must_use]
pub fn format_id(bytes: &[u8]) -> String {
    Uuid::from_slice(bytes).map_or_else(|_| "unknown".to_owned(), |id| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_fixed_length_identifiers() {
        assert_eq!(new_message_id().len(), crate::limits::MESSAGE_ID_BYTES);
        assert_eq!(new_challenge().len(), CHALLENGE_BYTES);
    }

    #[test]
    fn derives_a_stable_device_id_from_a_public_key() {
        let key = [7_u8; 32];

        let id = derive_device_id(&key);

        assert_eq!(id.len(), DEVICE_ID_BYTES);
        assert_eq!(id, derive_device_id(&key), "derivation must be stable");
        assert_ne!(id, key, "the identifier is derived, not the key itself");
        assert_ne!(id, derive_device_id(&[8_u8; 32]));
    }

    #[test]
    fn formats_identifiers_for_tracing() {
        let id = new_message_id();

        assert_eq!(
            format_id(&id),
            Uuid::from_slice(&id).expect("uuid").to_string()
        );
        assert_eq!(format_id(&[0x01]), "unknown");
    }
}
