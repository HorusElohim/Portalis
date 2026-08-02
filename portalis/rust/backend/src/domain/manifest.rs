use std::collections::BTreeMap;

use ed25519_dalek::Signature;

use super::identity::{DeviceId, DeviceIdentity};

/// The BitTorrent info-hash of a single media item's torrent (SHA-1, per
/// the BitTorrent spec — 20 bytes, distinct from the 32-byte blake3 hashes
/// used elsewhere in this crate for rendezvous keys and thumbnails).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct InfoHash([u8; 20]);

impl InfoHash {
    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> [u8; 20] {
        self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Debug for InfoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InfoHash({}…)", &self.to_hex()[..8])
    }
}

/// One media item, added to a collection by a specific device and signed
/// by it. Immutable once created — see [`Manifest`] for why.
#[derive(Clone)]
pub(crate) struct ManifestEntry {
    pub info_hash: InfoHash,
    pub name: String,
    pub thumbnail_hash: Option<[u8; 32]>,
    pub added_by: DeviceId,
    pub added_at_unix_ms: i64,
    signature: Signature,
}

impl ManifestEntry {
    /// Every field except the signature itself — what actually gets signed.
    fn signing_payload(
        info_hash: &InfoHash,
        name: &str,
        thumbnail_hash: Option<&[u8; 32]>,
        added_by: &DeviceId,
        added_at_unix_ms: i64,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&info_hash.as_bytes());
        buf.extend_from_slice(name.as_bytes());
        if let Some(hash) = thumbnail_hash {
            buf.extend_from_slice(hash);
        }
        buf.extend_from_slice(&added_by.as_bytes());
        buf.extend_from_slice(&added_at_unix_ms.to_le_bytes());
        buf
    }

    pub fn new_signed(
        info_hash: InfoHash,
        name: String,
        thumbnail_hash: Option<[u8; 32]>,
        identity: &DeviceIdentity,
        added_at_unix_ms: i64,
    ) -> Self {
        let added_by = identity.device_id();
        let payload = Self::signing_payload(
            &info_hash,
            &name,
            thumbnail_hash.as_ref(),
            &added_by,
            added_at_unix_ms,
        );
        let signature = identity.sign(&payload);
        Self {
            info_hash,
            name,
            thumbnail_hash,
            added_by,
            added_at_unix_ms,
            signature,
        }
    }

    /// Verify this entry was really signed by `added_by`. Called on every
    /// entry before it's allowed into a [`Manifest`] — this is what stops a
    /// peer from forging entries as someone else.
    pub fn verify(&self) -> bool {
        let payload = Self::signing_payload(
            &self.info_hash,
            &self.name,
            self.thumbnail_hash.as_ref(),
            &self.added_by,
            self.added_at_unix_ms,
        );
        self.added_by.verify(&payload, &self.signature)
    }

    /// Reconstructs an already-signed entry — for loading one back from
    /// persisted storage, or receiving one from a peer during manifest
    /// sync. Unlike `new_signed`, this does not compute a fresh signature;
    /// call `.verify()` afterward if the signature's authenticity still
    /// needs checking (always true for anything arriving from a peer).
    pub fn from_signed_parts(
        info_hash: InfoHash,
        name: String,
        thumbnail_hash: Option<[u8; 32]>,
        added_by: DeviceId,
        added_at_unix_ms: i64,
        signature: Signature,
    ) -> Self {
        Self {
            info_hash,
            name,
            thumbnail_hash,
            added_by,
            added_at_unix_ms,
            signature,
        }
    }

    /// The raw signature bytes — the counterpart to `from_signed_parts`,
    /// for persisting/transmitting an entry as-is.
    pub fn signature_bytes(&self) -> [u8; 64] {
        self.signature.to_bytes()
    }
}

/// A collection's set of media items: add-only, mergeable from any peer, no
/// central authority — a grow-only-set CRDT. Keying by info-hash means the
/// same media item added independently by two peers naturally
/// de-duplicates. Entries are only ever added after signature verification,
/// so the merged result can't contain forged authorship regardless of
/// which peer it arrived from.
///
/// Removal/moderation isn't modeled yet — see the backend README's open
/// questions; a grow-only set has no native "take something out."
#[derive(Clone, Default)]
pub(crate) struct Manifest {
    entries: BTreeMap<InfoHash, ManifestEntry>,
}

impl Manifest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one entry, after verifying its signature. Returns `false` (and
    /// does not insert) for unsigned/forged/tampered entries.
    pub fn add(&mut self, entry: ManifestEntry) -> bool {
        if !entry.verify() {
            return false;
        }
        let key = entry.info_hash;
        self.entries.entry(key).or_insert(entry);
        true
    }

    /// Not called in production yet — `apply_message` adds entries one at a
    /// time so it can name the one it refused. Kept because the four tests
    /// below are the proof that this model converges at all, and step 4 of
    /// docs/future-engine.md makes this the path production takes.
    #[allow(dead_code)]
    /// CRDT merge: union of both sets, keyed by info-hash. Commutative,
    /// associative, and idempotent — merging the same manifest twice, or in
    /// either order, converges to the same result. Invalid entries in
    /// `other` are silently dropped rather than poisoning the merge.
    pub fn merge(&mut self, other: &Manifest) {
        for entry in other.entries.values() {
            self.add(entry.clone());
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &ManifestEntry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only reach into the map, so `contains` need not exist in the
    /// production API for the sake of an assertion.
    impl Manifest {
        fn holds(&self, info_hash: [u8; 20]) -> bool {
            self.entries.contains_key(&InfoHash::from_bytes(info_hash))
        }
    }

    fn entry(identity: &DeviceIdentity, seed: u8) -> ManifestEntry {
        ManifestEntry::new_signed(
            InfoHash::from_bytes([seed; 20]),
            format!("media-{seed}"),
            None,
            identity,
            1_000 + seed as i64,
        )
    }

    #[test]
    fn add_accepts_validly_signed_entry() {
        let identity = DeviceIdentity::generate();
        let mut manifest = Manifest::new();

        assert!(manifest.add(entry(&identity, 1)));
        assert_eq!(manifest.len(), 1);
    }

    #[test]
    fn add_rejects_tampered_entry() {
        let identity = DeviceIdentity::generate();
        let mut tampered = entry(&identity, 1);
        tampered.name = "not what was signed".into();
        let mut manifest = Manifest::new();

        assert!(!manifest.add(tampered));
        assert_eq!(manifest.len(), 0);
    }

    #[test]
    fn add_is_idempotent_for_same_info_hash() {
        let identity = DeviceIdentity::generate();
        let mut manifest = Manifest::new();

        manifest.add(entry(&identity, 1));
        manifest.add(entry(&identity, 1));

        assert_eq!(manifest.len(), 1);
    }

    #[test]
    fn merge_unions_two_manifests() {
        let identity = DeviceIdentity::generate();
        let mut a = Manifest::new();
        a.add(entry(&identity, 1));
        let mut b = Manifest::new();
        b.add(entry(&identity, 2));

        a.merge(&b);

        assert_eq!(a.len(), 2);
        assert!(a.holds([1; 20]));
        assert!(a.holds([2; 20]));
    }

    #[test]
    fn merge_is_commutative() {
        let identity = DeviceIdentity::generate();
        let mut a = Manifest::new();
        a.add(entry(&identity, 1));
        let mut b = Manifest::new();
        b.add(entry(&identity, 2));

        let mut a_then_b = a.clone();
        a_then_b.merge(&b);
        let mut b_then_a = b.clone();
        b_then_a.merge(&a);

        assert_eq!(a_then_b.len(), b_then_a.len());
        for entry in a_then_b.entries() {
            assert!(b_then_a.holds(entry.info_hash.as_bytes()));
        }
    }

    #[test]
    fn merge_is_idempotent() {
        let identity = DeviceIdentity::generate();
        let mut a = Manifest::new();
        a.add(entry(&identity, 1));
        let b = a.clone();

        a.merge(&b);
        a.merge(&b);

        assert_eq!(a.len(), 1);
    }

    #[test]
    fn from_signed_parts_round_trips_and_still_verifies() {
        let identity = DeviceIdentity::generate();
        let original = entry(&identity, 1);

        let reconstructed = ManifestEntry::from_signed_parts(
            original.info_hash,
            original.name.clone(),
            original.thumbnail_hash,
            original.added_by,
            original.added_at_unix_ms,
            Signature::from_bytes(&original.signature_bytes()),
        );

        assert!(reconstructed.verify());
        assert_eq!(reconstructed.info_hash, original.info_hash);
    }

    #[test]
    fn merge_drops_forged_entries_from_the_other_side() {
        let identity = DeviceIdentity::generate();
        let mut forged = entry(&identity, 1);
        forged.name = "forged".into();
        let mut poisoned = Manifest::new();
        // Bypass `add`'s verification to simulate a malicious peer's
        // manifest arriving with a tampered entry already inside it.
        poisoned.entries.insert(forged.info_hash, forged);

        let mut clean = Manifest::new();
        clean.merge(&poisoned);

        assert_eq!(clean.len(), 0);
    }
}
