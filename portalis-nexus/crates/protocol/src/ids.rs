//! Binary identifier construction and log-safe formatting.

use uuid::Uuid;

use crate::limits::{CHALLENGE_BYTES, DEVICE_ID_BYTES, USER_ID_BYTES};

/// Bytes of entropy a `UUIDv7` needs beside its timestamp.
pub const UUID_V7_ENTROPY_BYTES: usize = 10;

/// Builds a `UUIDv7` from an explicit clock reading and entropy.
///
/// `new_message_id` reads the system clock and its own randomness, which makes
/// it untestable. Identifiers the server allocates for durable records go
/// through here instead, so a test can pin exactly which one is produced.
/// Time-ordered identifiers also keep `MongoDB` index writes local.
#[must_use]
pub fn user_id_from(
    now_unix_ms: u64,
    entropy: &[u8; UUID_V7_ENTROPY_BYTES],
) -> [u8; USER_ID_BYTES] {
    let mut id = [0_u8; USER_ID_BYTES];
    // 48-bit big-endian millisecond timestamp.
    id[..6].copy_from_slice(&now_unix_ms.to_be_bytes()[2..]);
    // Version 7 in the high nibble, then entropy.
    id[6] = 0x70 | (entropy[0] & 0x0f);
    id[7] = entropy[1];
    // RFC 4122 variant in the top two bits, then entropy.
    id[8] = 0x80 | (entropy[2] & 0x3f);
    id[9..].copy_from_slice(&entropy[3..]);
    id
}

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
    fn builds_time_ordered_user_ids_from_injected_inputs() {
        let entropy = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let id = user_id_from(1_700_000_000_000, &entropy);

        let parsed = Uuid::from_slice(&id).expect("a valid UUID");
        assert_eq!(parsed.get_version_num(), 7);
        assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
        assert_eq!(
            id,
            user_id_from(1_700_000_000_000, &entropy),
            "generation must be deterministic"
        );

        // Time ordering: a later timestamp sorts after an earlier one.
        let later = user_id_from(1_700_000_000_001, &entropy);
        assert!(later > id);
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
