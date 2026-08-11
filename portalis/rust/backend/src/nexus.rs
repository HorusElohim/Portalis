//! Backend-owned entry point for the portable Nexus client.
//!
//! Keep this module below the Flutter bridge for now: protobuf envelopes and
//! transport handles are implementation details, while the eventual Dart API
//! should speak in collection/share operations.

use portalis_nexus_client::{DeviceSigner, NexusClient, TransportError};
use rand::{rngs::OsRng, RngCore};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::domain::identity::DeviceIdentity;

/// The two independent device keys the Nexus workflow needs.
///
/// The existing Ed25519 identity remains the signing authority used by the
/// legacy collection model. X25519 is generated beside it and only receives
/// sealed share keys; neither private key crosses this backend boundary.
pub(crate) struct NexusIdentity {
    signing: DeviceIdentity,
    encryption: StaticSecret,
}

impl NexusIdentity {
    pub(crate) fn generate(signing: DeviceIdentity) -> Self {
        let mut encryption = [0_u8; 32];
        OsRng.fill_bytes(&mut encryption);
        Self {
            signing,
            encryption: StaticSecret::from(encryption),
        }
    }

    pub(crate) fn from_parts(signing: DeviceIdentity, encryption: [u8; 32]) -> Self {
        Self {
            signing,
            encryption: StaticSecret::from(encryption),
        }
    }

    pub(crate) fn signing_identity(&self) -> &DeviceIdentity {
        &self.signing
    }

    pub(crate) fn into_signing_identity(self) -> DeviceIdentity {
        self.signing
    }

    pub(crate) fn encryption_secret(&self) -> [u8; 32] {
        self.encryption.to_bytes()
    }
}

impl DeviceSigner for NexusIdentity {
    fn public_key(&self) -> [u8; 32] {
        self.signing.public_key()
    }

    fn encryption_public_key(&self) -> [u8; 32] {
        PublicKey::from(&self.encryption).to_bytes()
    }

    fn sign(&self, payload: &[u8]) -> [u8; 64] {
        self.signing.sign(payload).to_bytes()
    }
}

/// Opens the supervised client used by the online collection workflow.
#[allow(dead_code)]
pub(crate) async fn connect(endpoint: &str) -> Result<NexusClient, TransportError> {
    NexusClient::connect(endpoint).await
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    use super::*;

    #[test]
    fn existing_signing_identity_drives_the_portable_client() {
        let identity = NexusIdentity::generate(DeviceIdentity::from_bytes(&[7; 32]));
        let payload = b"nexus registration payload";

        assert_eq!(
            identity.public_key(),
            identity.signing_identity().public_key()
        );
        assert_ne!(DeviceSigner::device_id(&identity), identity.public_key());
        assert!(VerifyingKey::from_bytes(&identity.public_key())
            .unwrap()
            .verify(payload, &Signature::from_bytes(&identity.sign(payload)))
            .is_ok());
    }

    #[test]
    fn encryption_key_round_trips_independently() {
        let identity = NexusIdentity::from_parts(DeviceIdentity::from_bytes(&[9; 32]), [11; 32]);
        let restored = NexusIdentity::from_parts(
            DeviceIdentity::from_bytes(&identity.signing_identity().to_bytes()),
            identity.encryption_secret(),
        );

        assert_eq!(restored.public_key(), identity.public_key());
        assert_eq!(
            restored.encryption_public_key(),
            identity.encryption_public_key()
        );
    }
}
