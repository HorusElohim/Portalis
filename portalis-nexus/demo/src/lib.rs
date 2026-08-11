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
use x25519_dalek::{PublicKey, StaticSecret};

/// How many bytes [`DemoDevice::load_or_create`] keeps on disk: one signing
/// seed and one encryption secret, side by side.
pub const KEY_FILE_BYTES: usize = DEVICE_KEY_BYTES + ENCRYPTION_KEY_BYTES;

/// A device's two keys kept in a file, the way an app keeps its identity.
///
/// Signing and encryption are separate keypairs on separate curves, and
/// neither is derived from the other: one proves who is acting, the other
/// receives share keys sealed to this device.
pub struct DemoDevice {
    key: SigningKey,
    encryption: StaticSecret,
}

impl DemoDevice {
    /// Generates a device that lives only for this process.
    #[must_use]
    pub fn ephemeral(seed: u8) -> Self {
        Self {
            key: SigningKey::from_bytes(&[seed; DEVICE_KEY_BYTES]),
            // Any bytes distinct from the signing seed will do here; a real
            // device draws this from the operating system, as below.
            encryption: StaticSecret::from([seed.wrapping_add(0x80); ENCRYPTION_KEY_BYTES]),
        }
    }

    /// Loads the keys at `path`, creating them on first run.
    ///
    /// Returns whether they were newly created, which tells the caller
    /// whether to register or to authenticate.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the key file cannot be read or written.
    pub fn load_or_create(path: &Path) -> io::Result<(Self, bool)> {
        if let Ok(stored) = fs::read(path) {
            let bytes: [u8; KEY_FILE_BYTES] = stored.as_slice().try_into().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{} holds {} bytes, not {KEY_FILE_BYTES}; delete it to start over \
                         as a new device",
                        path.display(),
                        stored.len()
                    ),
                )
            })?;
            return Ok((Self::from_bytes(&bytes), false));
        }

        let mut signing = [0_u8; DEVICE_KEY_BYTES];
        let mut encryption = [0_u8; ENCRYPTION_KEY_BYTES];
        getrandom_seed(&mut signing);
        getrandom_seed(&mut encryption);
        let mut bytes = [0_u8; KEY_FILE_BYTES];
        bytes[..DEVICE_KEY_BYTES].copy_from_slice(&signing);
        bytes[DEVICE_KEY_BYTES..].copy_from_slice(&encryption);
        fs::write(path, bytes)?;
        Ok((Self::from_bytes(&bytes), true))
    }

    fn from_bytes(bytes: &[u8; KEY_FILE_BYTES]) -> Self {
        let mut signing = [0_u8; DEVICE_KEY_BYTES];
        let mut encryption = [0_u8; ENCRYPTION_KEY_BYTES];
        signing.copy_from_slice(&bytes[..DEVICE_KEY_BYTES]);
        encryption.copy_from_slice(&bytes[DEVICE_KEY_BYTES..]);
        Self {
            key: SigningKey::from_bytes(&signing),
            encryption: StaticSecret::from(encryption),
        }
    }

    /// The identifier the server derives from this device's public key.
    #[must_use]
    pub fn device_id(&self) -> [u8; 32] {
        derive_device_id(&self.public_key())
    }

    /// The private half of the encryption keypair, which opens envelopes
    /// sealed to this device. A real app keeps this in a keychain and never
    /// hands it out; the demo needs it to show a share key being recovered.
    #[must_use]
    pub fn encryption_secret(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        self.encryption.to_bytes()
    }
}

impl DeviceSigner for DemoDevice {
    fn public_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.key.verifying_key().to_bytes()
    }

    fn encryption_public_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        PublicKey::from(&self.encryption).to_bytes()
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
