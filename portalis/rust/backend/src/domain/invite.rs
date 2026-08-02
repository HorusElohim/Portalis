use rand::rngs::OsRng;
use rand::RngCore;

/// Domain separation tag for rendezvous-key derivation, so this hash can
/// never collide with a hash computed for some other purpose elsewhere in
/// the codebase, even given the same input bytes.
const RENDEZVOUS_DOMAIN: &[u8] = b"portalis.rendezvous.v1";

/// A random secret minted when a collection is created, encoded into the
/// invite link/QR. Knowing it is what makes you a collaborator. It never
/// touches the DHT directly — only its derived [`RendezvousKey`] does, so
/// the (public) DHT never sees anything an outside observer could invert
/// back into the secret.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct InviteSecret([u8; 32]);

impl InviteSecret {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        let bytes = hex::decode(s)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("invite secret must be 32 bytes"))?;
        Ok(Self(arr))
    }

    /// Derive the DHT rendezvous key collaborators announce/look-up under.
    /// One-way (a hash, not an encoding) — the DHT is public infrastructure,
    /// but this key is unguessable without the secret.
    pub fn derive_rendezvous_key(&self) -> RendezvousKey {
        let mut hasher = blake3::Hasher::new();
        hasher.update(RENDEZVOUS_DOMAIN);
        hasher.update(&self.0);
        RendezvousKey(*hasher.finalize().as_bytes())
    }
}

/// The public, DHT-visible key peers announce/look-up under for a given
/// collection. See [`InviteSecret::derive_rendezvous_key`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RendezvousKey([u8; 32]);

impl std::fmt::Debug for RendezvousKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RendezvousKey({}…)", &self.to_hex()[..8])
    }
}

impl RendezvousKey {
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic() {
        let secret = InviteSecret::generate();

        assert_eq!(
            secret.derive_rendezvous_key().to_hex(),
            secret.derive_rendezvous_key().to_hex()
        );
    }

    #[test]
    fn different_secrets_derive_different_keys() {
        let a = InviteSecret::generate();
        let b = InviteSecret::generate();

        assert_ne!(a.derive_rendezvous_key(), b.derive_rendezvous_key());
    }

    #[test]
    fn hex_round_trips() {
        let secret = InviteSecret::generate();

        assert_eq!(
            InviteSecret::from_hex(&secret.to_hex()).unwrap().to_hex(),
            secret.to_hex()
        );
    }

    #[test]
    fn rejects_wrong_length_hex() {
        assert!(InviteSecret::from_hex("abcd").is_err());
    }
}
