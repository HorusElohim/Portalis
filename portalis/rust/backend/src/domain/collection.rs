use uuid::Uuid;

use super::collaborator::Collaborator;
use super::invite::{InviteSecret, RendezvousKey};
use super::manifest::{Manifest, ManifestEntry};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CollectionId(Uuid);

impl CollectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses an id previously produced by `Display` — for reloading a
    /// persisted collection back with its *same* id, unlike `join`'s fresh
    /// `CollectionId::new()` (see `from_parts`).
    pub fn from_string(s: &str) -> anyhow::Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl std::fmt::Display for CollectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A named, growable album: an invite secret, a set of collaborators, and a
/// [`Manifest`] of media items. See the backend README for why this is
/// *not* a single BitTorrent torrent.
pub(crate) struct Collection {
    pub id: CollectionId,
    pub name: String,
    invite_secret: InviteSecret,
    pub collaborators: Vec<Collaborator>,
    manifest: Manifest,
}

impl Collection {
    pub fn new(name: String) -> Self {
        Self {
            id: CollectionId::new(),
            name,
            invite_secret: InviteSecret::generate(),
            collaborators: Vec::new(),
            manifest: Manifest::new(),
        }
    }

    /// Reconstruct a collection this device is joining, from an invite
    /// secret someone else minted (decoded from a QR/link).
    pub fn join(name: String, invite_secret: InviteSecret) -> Self {
        Self {
            id: CollectionId::new(),
            name,
            invite_secret,
            collaborators: Vec::new(),
            manifest: Manifest::new(),
        }
    }

    /// Reconstructs a collection from persisted state — the counterpart to
    /// `new`/`join`, used when loading `collections.json` back in after an
    /// app restart. Unlike `join`, this restores the *exact same*
    /// `CollectionId` rather than minting a fresh local handle, since this
    /// really is continuing the same local record, not creating a new one.
    pub fn from_parts(
        id: CollectionId,
        name: String,
        invite_secret: InviteSecret,
        collaborators: Vec<Collaborator>,
        manifest: Manifest,
    ) -> Self {
        Self {
            id,
            name,
            invite_secret,
            collaborators,
            manifest,
        }
    }

    pub fn rendezvous_key(&self) -> RendezvousKey {
        self.invite_secret.derive_rendezvous_key()
    }

    pub fn invite_secret_hex(&self) -> String {
        self.invite_secret.to_hex()
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Merge manifest entries received from a peer during the join/sync
    /// handshake described in the backend README.
    pub fn merge_manifest(&mut self, other: &Manifest) {
        self.manifest.merge(other);
    }

    /// Returns `true` iff the entry was validly signed and newly added.
    pub fn add_manifest_entry(&mut self, entry: ManifestEntry) -> bool {
        self.manifest.add(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::DeviceIdentity;
    use crate::domain::manifest::InfoHash;

    #[test]
    fn two_collections_get_different_ids_and_secrets() {
        let a = Collection::new("Iceland 2024".into());
        let b = Collection::new("Band Practice".into());

        assert_ne!(a.id, b.id);
        assert_ne!(a.invite_secret_hex(), b.invite_secret_hex());
    }

    #[test]
    fn joining_with_the_same_secret_yields_the_same_rendezvous_key() {
        let original = Collection::new("Family Reunion".into());
        let secret = InviteSecret::from_hex(&original.invite_secret_hex()).unwrap();

        let joined = Collection::join("Family Reunion".into(), secret);

        assert_eq!(
            original.rendezvous_key().to_hex(),
            joined.rendezvous_key().to_hex()
        );
        // Joining locally creates a distinct CollectionId — it's a
        // different device's local record of the same shared collection.
        assert_ne!(original.id, joined.id);
    }

    #[test]
    fn collection_id_round_trips_through_string() {
        let id = CollectionId::new();

        assert_eq!(CollectionId::from_string(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn from_parts_reconstructs_the_exact_same_id_and_manifest() {
        let identity = DeviceIdentity::generate();
        let original = Collection::new("Studio Shoot".into());
        let secret = InviteSecret::from_hex(&original.invite_secret_hex()).unwrap();
        let mut manifest = Manifest::new();
        manifest.add(ManifestEntry::new_signed(
            InfoHash::from_bytes([9; 20]),
            "RAW_9000".into(),
            None,
            &identity,
            7,
        ));

        let reloaded = Collection::from_parts(
            original.id,
            original.name.clone(),
            secret,
            Vec::new(),
            manifest,
        );

        assert_eq!(reloaded.id, original.id);
        assert_eq!(reloaded.manifest().len(), 1);
        assert_eq!(
            reloaded.rendezvous_key().to_hex(),
            original.rendezvous_key().to_hex()
        );
    }

    #[test]
    fn adding_media_updates_the_manifest() {
        let identity = DeviceIdentity::generate();
        let mut collection = Collection::new("Studio Shoot".into());
        let entry = ManifestEntry::new_signed(
            InfoHash::from_bytes([7; 20]),
            "RAW_3000".into(),
            None,
            &identity,
            42,
        );

        assert!(collection.add_manifest_entry(entry));
        assert_eq!(collection.manifest().len(), 1);
    }
}
