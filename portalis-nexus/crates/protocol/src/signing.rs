//! Deterministic, domain-separated payloads that devices sign.
//!
//! A signature is only as good as the bytes it covers. Every payload here is
//! built the same way: a context string that names the operation, then each
//! field length-prefixed. Length prefixes matter — plain concatenation lets a
//! signature over `("ab", "c")` be reinterpreted as one over `("a", "bc")`, so
//! a registration payload could be replayed as a different username.
//!
//! Binding the `ServerHello` context into every payload means a signature is
//! valid for exactly one challenge, on one connection, to one server.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::limits::{DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, SIGNATURE_BYTES};

/// Names the registration operation inside its signed payload.
pub const REGISTRATION_CONTEXT: &str = "portalis.protocol.v1/register-user";
/// Names the authentication operation inside its signed payload.
pub const AUTHENTICATION_CONTEXT: &str = "portalis.protocol.v1/authenticate-device";
/// Names the device-linking operation inside its signed payload.
pub const LINK_DEVICE_CONTEXT: &str = "portalis.protocol.v1/link-device";

/// The `ServerHello` facts that bind a signature to one connection attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionBinding<'a> {
    pub protocol_version: u32,
    /// The host the client believes it is talking to, so a signature captured
    /// by one deployment cannot be replayed against another.
    pub server_authority: &'a str,
    pub connection_id: &'a [u8],
    pub challenge: &'a [u8],
    pub server_time_unix_ms: u64,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SignatureError {
    #[error("device public key must contain exactly {DEVICE_KEY_BYTES} bytes, got {actual}")]
    InvalidKeyLength { actual: usize },
    #[error("device public key is not a valid Ed25519 point")]
    MalformedKey,
    #[error("signature must contain exactly {SIGNATURE_BYTES} bytes, got {actual}")]
    InvalidSignatureLength { actual: usize },
    #[error(
        "encryption public key must contain exactly {ENCRYPTION_KEY_BYTES} bytes, got {actual}"
    )]
    InvalidEncryptionKeyLength { actual: usize },
    #[error("signature does not match the signed payload")]
    Rejected,
}

/// Builds the bytes a device signs to claim a new username.
///
/// The encryption key is covered by the same signature as the signing key,
/// so neither can be substituted after the fact without invalidating it.
#[must_use]
pub fn registration_payload(
    binding: &SessionBinding<'_>,
    requested_username: &str,
    device_public_key: &[u8],
    encryption_public_key: &[u8],
) -> Vec<u8> {
    let mut payload = binding.encode(REGISTRATION_CONTEXT);
    push_field(&mut payload, requested_username.as_bytes());
    push_field(&mut payload, device_public_key);
    push_field(&mut payload, encryption_public_key);
    payload
}

/// Builds the bytes a device signs to prove it owns an authorized key.
#[must_use]
pub fn authentication_payload(binding: &SessionBinding<'_>, device_public_key: &[u8]) -> Vec<u8> {
    let mut payload = binding.encode(AUTHENTICATION_CONTEXT);
    push_field(&mut payload, device_public_key);
    payload
}

/// Builds the bytes an already-authorized device signs to approve a new one.
///
/// Durable rather than session-bound: it covers the server this approval is
/// valid for and the two keys it grants, not a connection or challenge, so it
/// can be produced once — even offline, from a scanned code — and submitted
/// by the candidate device whenever it next connects.
#[must_use]
pub fn link_device_payload(
    server_authority: &str,
    candidate_signing_public_key: &[u8],
    candidate_encryption_public_key: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::new();
    push_field(&mut payload, LINK_DEVICE_CONTEXT.as_bytes());
    push_field(&mut payload, server_authority.as_bytes());
    push_field(&mut payload, candidate_signing_public_key);
    push_field(&mut payload, candidate_encryption_public_key);
    payload
}

/// Verifies an Ed25519 signature over a payload built by this module.
///
/// # Errors
///
/// Returns [`SignatureError`] when the key or signature is malformed, or when
/// the signature does not cover exactly these bytes with this key.
pub fn verify_signature(
    device_public_key: &[u8],
    payload: &[u8],
    signature: &[u8],
) -> Result<(), SignatureError> {
    let key: [u8; DEVICE_KEY_BYTES] =
        device_public_key
            .try_into()
            .map_err(|_| SignatureError::InvalidKeyLength {
                actual: device_public_key.len(),
            })?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| SignatureError::MalformedKey)?;
    let signature: [u8; SIGNATURE_BYTES] =
        signature
            .try_into()
            .map_err(|_| SignatureError::InvalidSignatureLength {
                actual: signature.len(),
            })?;

    key.verify_strict(payload, &Signature::from_bytes(&signature))
        .map_err(|_| SignatureError::Rejected)
}

impl SessionBinding<'_> {
    fn encode(&self, context: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        push_field(&mut payload, context.as_bytes());
        push_field(&mut payload, &self.protocol_version.to_be_bytes());
        push_field(&mut payload, self.server_authority.as_bytes());
        push_field(&mut payload, self.connection_id);
        push_field(&mut payload, self.challenge);
        push_field(&mut payload, &self.server_time_unix_ms.to_be_bytes());
        payload
    }
}

/// Appends one length-prefixed field, keeping the encoding unambiguous.
fn push_field(payload: &mut Vec<u8>, field: &[u8]) {
    let length = u32::try_from(field.len()).unwrap_or(u32::MAX);
    payload.extend_from_slice(&length.to_be_bytes());
    payload.extend_from_slice(field);
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn binding<'a>(challenge: &'a [u8], connection_id: &'a [u8]) -> SessionBinding<'a> {
        SessionBinding {
            protocol_version: 1,
            server_authority: "nexus.portalis.test",
            connection_id,
            challenge,
            server_time_unix_ms: 1_700_000_000_000,
        }
    }

    const ENCRYPTION_KEY: [u8; ENCRYPTION_KEY_BYTES] = [5; ENCRYPTION_KEY_BYTES];

    #[test]
    fn accepts_a_registration_signature_over_its_own_payload() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let payload = registration_payload(
            &binding(&[1; 32], &[2; 16]),
            "ada",
            &public,
            &ENCRYPTION_KEY,
        );
        let signature = key.sign(&payload).to_bytes();

        assert_eq!(verify_signature(&public, &payload, &signature), Ok(()));
    }

    #[test]
    fn accepts_an_authentication_signature_over_its_own_payload() {
        let key = signing_key(9);
        let public = key.verifying_key().to_bytes();
        let payload = authentication_payload(&binding(&[3; 32], &[4; 16]), &public);
        let signature = key.sign(&payload).to_bytes();

        assert_eq!(verify_signature(&public, &payload, &signature), Ok(()));
    }

    #[test]
    fn separates_registration_from_authentication() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let session = binding(&[1; 32], &[2; 16]);
        let registration = registration_payload(&session, "ada", &public, &ENCRYPTION_KEY);
        let authentication = authentication_payload(&session, &public);

        assert_ne!(registration, authentication);
        // An authentication signature must not authorize a registration.
        let signature = key.sign(&authentication).to_bytes();
        assert_eq!(
            verify_signature(&public, &registration, &signature),
            Err(SignatureError::Rejected)
        );
    }

    #[test]
    fn binds_a_signature_to_one_challenge_connection_server_and_time() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let session = binding(&[1; 32], &[2; 16]);
        let payload = registration_payload(&session, "ada", &public, &ENCRYPTION_KEY);

        let other_challenge = registration_payload(
            &binding(&[9; 32], &[2; 16]),
            "ada",
            &public,
            &ENCRYPTION_KEY,
        );
        let other_connection = registration_payload(
            &binding(&[1; 32], &[9; 16]),
            "ada",
            &public,
            &ENCRYPTION_KEY,
        );
        let mut elsewhere = session;
        elsewhere.server_authority = "nexus.attacker.test";
        let other_server = registration_payload(&elsewhere, "ada", &public, &ENCRYPTION_KEY);
        let mut later = session;
        later.server_time_unix_ms += 1;
        let other_time = registration_payload(&later, "ada", &public, &ENCRYPTION_KEY);
        let mut downgraded = session;
        downgraded.protocol_version = 2;
        let other_version = registration_payload(&downgraded, "ada", &public, &ENCRYPTION_KEY);
        let other_encryption_key =
            registration_payload(&session, "ada", &public, &[9; ENCRYPTION_KEY_BYTES]);

        for changed in [
            other_challenge,
            other_connection,
            other_server,
            other_time,
            other_version,
            other_encryption_key,
        ] {
            assert_ne!(payload, changed);
        }
    }

    #[test]
    fn length_prefixes_keep_adjacent_fields_unambiguous() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let session = binding(&[1; 32], &[2; 16]);

        // Without length prefixes these two would encode to the same bytes.
        assert_ne!(
            registration_payload(&session, "ab", &public, &ENCRYPTION_KEY),
            registration_payload(&session, "a", &[b'b'; 32], &ENCRYPTION_KEY)
        );
    }

    #[test]
    fn rejects_a_signature_from_another_device() {
        let signer = signing_key(7);
        let impostor = signing_key(8);
        let public = signer.verifying_key().to_bytes();
        let payload = registration_payload(
            &binding(&[1; 32], &[2; 16]),
            "ada",
            &public,
            &ENCRYPTION_KEY,
        );
        let signature = signer.sign(&payload).to_bytes();

        assert_eq!(
            verify_signature(&impostor.verifying_key().to_bytes(), &payload, &signature),
            Err(SignatureError::Rejected)
        );
    }

    #[test]
    fn rejects_malformed_keys_and_signatures() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let payload = registration_payload(
            &binding(&[1; 32], &[2; 16]),
            "ada",
            &public,
            &ENCRYPTION_KEY,
        );
        let signature = key.sign(&payload).to_bytes();

        assert_eq!(
            verify_signature(&public[..31], &payload, &signature),
            Err(SignatureError::InvalidKeyLength { actual: 31 })
        );
        // Not every 32-byte string is a curve point; this one fails to
        // decompress, unlike most random bytes.
        let mut off_curve = [0_u8; 32];
        off_curve[0] = 2;
        off_curve[31] = 0x80;
        assert_eq!(
            verify_signature(&off_curve, &payload, &signature),
            Err(SignatureError::MalformedKey)
        );
        assert_eq!(
            verify_signature(&public, &payload, &signature[..63]),
            Err(SignatureError::InvalidSignatureLength { actual: 63 })
        );
    }

    #[test]
    fn accepts_a_link_device_signature_over_its_own_payload() {
        let approver = signing_key(11);
        let approver_public = approver.verifying_key().to_bytes();
        let candidate_signing = signing_key(12).verifying_key().to_bytes();
        let payload =
            link_device_payload("nexus.portalis.test", &candidate_signing, &ENCRYPTION_KEY);
        let signature = approver.sign(&payload).to_bytes();

        assert_eq!(
            verify_signature(&approver_public, &payload, &signature),
            Ok(())
        );
    }

    #[test]
    fn a_link_device_approval_is_durable_across_connections() {
        // No connection_id, challenge, or server_time enters the payload at
        // all: the same bytes are valid however many times, and on whatever
        // connection, the candidate device submits them.
        let candidate_signing = signing_key(12).verifying_key().to_bytes();
        let first = link_device_payload("nexus.portalis.test", &candidate_signing, &ENCRYPTION_KEY);
        let second =
            link_device_payload("nexus.portalis.test", &candidate_signing, &ENCRYPTION_KEY);

        assert_eq!(first, second);
    }

    #[test]
    fn a_link_device_approval_is_bound_to_its_server_and_both_candidate_keys() {
        let candidate_signing = signing_key(12).verifying_key().to_bytes();
        let payload =
            link_device_payload("nexus.portalis.test", &candidate_signing, &ENCRYPTION_KEY);

        let other_server =
            link_device_payload("nexus.attacker.test", &candidate_signing, &ENCRYPTION_KEY);
        let other_signing_key = link_device_payload(
            "nexus.portalis.test",
            &[9; DEVICE_KEY_BYTES],
            &ENCRYPTION_KEY,
        );
        let other_encryption_key = link_device_payload(
            "nexus.portalis.test",
            &candidate_signing,
            &[9; ENCRYPTION_KEY_BYTES],
        );

        for changed in [other_server, other_signing_key, other_encryption_key] {
            assert_ne!(payload, changed);
        }
    }

    #[test]
    fn separates_link_device_from_registration_and_authentication() {
        let key = signing_key(7);
        let public = key.verifying_key().to_bytes();
        let session = binding(&[1; 32], &[2; 16]);
        let registration = registration_payload(&session, "ada", &public, &ENCRYPTION_KEY);
        let authentication = authentication_payload(&session, &public);
        let link_device = link_device_payload("nexus.portalis.test", &public, &ENCRYPTION_KEY);

        assert_ne!(link_device, registration);
        assert_ne!(link_device, authentication);
    }
}
