use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

/// A device's public identity — stable across IP/network changes, used as
/// the "who" in collaborator lists and manifest entry authorship.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(VerifyingKey);

impl DeviceId {
    pub fn as_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        let bytes = hex::decode(s)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("device id must be 32 bytes"))?;
        Ok(Self(VerifyingKey::from_bytes(&arr)?))
    }

    /// Verify a message was signed by this device.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        self.0.verify(message, signature).is_ok()
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({}…)", &self.to_hex()[..8])
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// A device's full keypair. The private half never leaves this type —
/// everything else in the domain only ever sees a [`DeviceId`] or a
/// `Signature`, never the signing key itself. Persistence is a `KeyStore`
/// adapter's job, not this type's — `to_bytes`/`from_bytes` are the seam.
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(bytes),
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId(self.signing_key.verifying_key())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let identity = DeviceIdentity::generate();
        let message = b"hello swarm";
        let signature = identity.sign(message);

        assert!(identity.device_id().verify(message, &signature));
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let identity = DeviceIdentity::generate();
        let signature = identity.sign(b"original");

        assert!(!identity.device_id().verify(b"tampered", &signature));
    }

    #[test]
    fn verify_rejects_wrong_signer() {
        let signer = DeviceIdentity::generate();
        let impostor = DeviceIdentity::generate();
        let message = b"who signed this?";
        let signature = signer.sign(message);

        assert!(!impostor.device_id().verify(message, &signature));
    }

    #[test]
    fn device_id_hex_round_trips() {
        let identity = DeviceIdentity::generate();
        let id = identity.device_id();

        assert_eq!(DeviceId::from_hex(&id.to_hex()).unwrap(), id);
    }

    #[test]
    fn identity_bytes_round_trip_to_same_device_id() {
        let identity = DeviceIdentity::generate();
        let bytes = identity.to_bytes();
        let restored = DeviceIdentity::from_bytes(&bytes);

        assert_eq!(identity.device_id(), restored.device_id());
    }
}
