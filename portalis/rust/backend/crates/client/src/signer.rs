//! How a caller proves it owns a device key.
//!
//! The client never holds key material. A caller implements this trait over
//! whatever already guards its Ed25519 key — a keychain, a secure enclave, or
//! the existing Portalis identity — and the client only ever sees the public
//! key and finished signatures.

use portalis_nexus_protocol::{
    DEVICE_ID_BYTES, DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, SIGNATURE_BYTES, derive_device_id,
};

pub trait DeviceSigner: Send + Sync {
    /// The Ed25519 public key identifying this device.
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES];

    /// The X25519 public key this device receives encrypted share-key
    /// envelopes at. Registered alongside the signing key, but never used to
    /// prove anything itself — only `sign` does that.
    fn encryption_public_key(&self) -> [u8; ENCRYPTION_KEY_BYTES];

    /// Signs a payload built by the protocol crate.
    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES];

    /// The stable identifier Nexus derives from this signing key.
    ///
    /// Callers use this rather than duplicating the domain-separated BLAKE3
    /// derivation owned by the protocol contract.
    #[must_use]
    fn device_id(&self) -> [u8; DEVICE_ID_BYTES] {
        derive_device_id(&self.public_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSigner;

    impl DeviceSigner for FixedSigner {
        fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
            [7; DEVICE_KEY_BYTES]
        }

        fn encryption_public_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
            [9; ENCRYPTION_KEY_BYTES]
        }

        fn sign(&self, _payload: &[u8]) -> [u8; SIGNATURE_BYTES] {
            [0; SIGNATURE_BYTES]
        }
    }

    #[test]
    fn device_id_uses_the_protocol_derivation() {
        let signer = FixedSigner;

        assert_eq!(signer.device_id(), derive_device_id(&signer.public_key()));
        assert_ne!(
            signer.encryption_public_key().as_slice(),
            signer.public_key().as_slice(),
            "the two keys are independent, on different curves"
        );
        assert_eq!(signer.sign(b"payload").len(), SIGNATURE_BYTES);
    }
}
