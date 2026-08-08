//! Binary identifier construction and log-safe formatting.

use uuid::Uuid;

use crate::limits::CHALLENGE_BYTES;

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
    fn formats_identifiers_for_tracing() {
        let id = new_message_id();

        assert_eq!(
            format_id(&id),
            Uuid::from_slice(&id).expect("uuid").to_string()
        );
        assert_eq!(format_id(&[0x01]), "unknown");
    }
}
