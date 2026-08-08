//! How a caller proves it owns a device key.
//!
//! The client never holds key material. A caller implements this trait over
//! whatever already guards its Ed25519 key — a keychain, a secure enclave, or
//! the existing Portalis identity — and the client only ever sees the public
//! key and finished signatures.

use portalis_nexus_protocol::{DEVICE_KEY_BYTES, SIGNATURE_BYTES};

pub trait DeviceSigner: Send + Sync {
    /// The Ed25519 public key identifying this device.
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES];

    /// Signs a payload built by the protocol crate.
    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES];
}
