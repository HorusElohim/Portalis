//! Shared pieces for the Portalis Nexus demo.
//!
//! The interesting part for anyone integrating the client is [`DemoDevice`]:
//! it shows what a [`DeviceSigner`] implementation looks like. The client
//! never sees the private key — only the public key and finished signatures —
//! so a real application can back this with a keychain or secure enclave
//! instead of a file.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::DeviceSigner;
use portalis_nexus_protocol::{
    DEVICE_KEY_BYTES, ENCRYPTION_KEY_BYTES, SIGNATURE_BYTES, derive_device_id, format_id,
};

/// An Ed25519 device key kept in a file, the way an app keeps its identity.
pub struct DemoDevice {
    key: SigningKey,
    /// Registered alongside the signing key so the server has somewhere to
    /// address encrypted share-key envelopes. Not a real X25519 keypair —
    /// this demo does not yet exercise share delivery — and not persisted,
    /// since only registration and linking ever submit it.
    encryption_public_key: [u8; ENCRYPTION_KEY_BYTES],
}

impl DemoDevice {
    /// Generates a device key that lives only for this process.
    #[must_use]
    pub fn ephemeral(seed: u8) -> Self {
        Self {
            key: SigningKey::from_bytes(&[seed; DEVICE_KEY_BYTES]),
            encryption_public_key: [seed; ENCRYPTION_KEY_BYTES],
        }
    }

    /// Loads the key at `path`, creating one on first run.
    ///
    /// Returns whether the key was newly created, which tells the caller
    /// whether to register or to authenticate.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the key file cannot be read or written.
    pub fn load_or_create(path: &Path) -> io::Result<(Self, bool)> {
        if let Ok(stored) = fs::read(path) {
            let seed: [u8; DEVICE_KEY_BYTES] = stored.as_slice().try_into().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} is not a {DEVICE_KEY_BYTES}-byte key", path.display()),
                )
            })?;
            let mut encryption_public_key = [0_u8; ENCRYPTION_KEY_BYTES];
            getrandom_seed(&mut encryption_public_key);
            return Ok((
                Self {
                    key: SigningKey::from_bytes(&seed),
                    encryption_public_key,
                },
                false,
            ));
        }

        let mut seed = [0_u8; DEVICE_KEY_BYTES];
        getrandom_seed(&mut seed);
        fs::write(path, seed)?;
        let mut encryption_public_key = [0_u8; ENCRYPTION_KEY_BYTES];
        getrandom_seed(&mut encryption_public_key);
        Ok((
            Self {
                key: SigningKey::from_bytes(&seed),
                encryption_public_key,
            },
            true,
        ))
    }

    /// The identifier the server derives from this device's public key.
    #[must_use]
    pub fn device_id(&self) -> [u8; 32] {
        derive_device_id(&self.public_key())
    }
}

impl DeviceSigner for DemoDevice {
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.key.verifying_key().to_bytes()
    }

    fn encryption_public_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        self.encryption_public_key
    }

    fn sign(&self, payload: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.key.sign(payload).to_bytes()
    }
}

/// Fills a seed from the operating system, without adding a dependency here.
fn getrandom_seed(seed: &mut [u8; DEVICE_KEY_BYTES]) {
    // `new_challenge` draws from the OS random source and returns 32 bytes.
    seed.copy_from_slice(&portalis_nexus_protocol::new_challenge());
}

/// Renders an identifier short enough to read in a terminal.
#[must_use]
pub fn short(bytes: &[u8]) -> String {
    if bytes.len() == 16 {
        return format_id(bytes);
    }
    let mut rendered = String::new();
    for byte in bytes.iter().take(6) {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered.push('…');
    rendered
}
