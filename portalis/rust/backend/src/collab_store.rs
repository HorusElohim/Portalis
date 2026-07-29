//! Local persistence for collab collections (`collections.json`) —
//! deliberately its own top-level module, *not* listed in
//! `flutter_rust_bridge`'s `--rust-input` (see `tool/frb_build.sh`), for
//! the same reason `domain` isn't listed there either: FRB's codegen
//! bridges every type/function whose *own* signature is textually present
//! within a listed module's subtree, regardless of Rust visibility. A
//! private `PersistedCollection` struct living inside `crate::collab`
//! would get swept up and FRB would try (and fail — its fields aren't
//! `pub`) to generate bridging code for it, even though nothing outside
//! this crate ever needs it. Keeping the persisted-DTO types and the
//! functions that mention them by name in a module `collab.rs` never
//! imports into `--rust-input` sidesteps the problem entirely.

use std::path::PathBuf;

use anyhow::Context;
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};

use crate::domain::collaborator::{Collaborator, Role};
use crate::domain::collection::{Collection, CollectionId};
use crate::domain::identity::DeviceId;
use crate::domain::invite::InviteSecret;
use crate::domain::manifest::{InfoHash, Manifest, ManifestEntry};

#[derive(Serialize, Deserialize)]
struct PersistedManifestEntry {
    info_hash_hex: String,
    name: String,
    thumbnail_hash_hex: Option<String>,
    added_by_hex: String,
    added_at_unix_ms: i64,
    signature_hex: String,
}

#[derive(Serialize, Deserialize)]
struct PersistedCollaborator {
    device_id_hex: String,
    display_name: String,
    is_admin: bool,
    joined_at_unix_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct PersistedCollection {
    id: String,
    name: String,
    invite_secret_hex: String,
    collaborators: Vec<PersistedCollaborator>,
    manifest: Vec<PersistedManifestEntry>,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedStore {
    collections: Vec<PersistedCollection>,
}

fn store_file() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Portalis").join("collections.json")
}

fn to_persisted(collection: &Collection) -> PersistedCollection {
    PersistedCollection {
        id: collection.id.to_string(),
        name: collection.name.clone(),
        invite_secret_hex: collection.invite_secret_hex(),
        collaborators: collection
            .collaborators
            .iter()
            .map(|c| PersistedCollaborator {
                device_id_hex: c.device_id.to_hex(),
                display_name: c.display_name.clone(),
                is_admin: c.is_admin(),
                joined_at_unix_ms: c.joined_at_unix_ms,
            })
            .collect(),
        manifest: collection
            .manifest()
            .entries()
            .map(|e| PersistedManifestEntry {
                info_hash_hex: e.info_hash.to_hex(),
                name: e.name.clone(),
                thumbnail_hash_hex: e.thumbnail_hash.map(hex::encode),
                added_by_hex: e.added_by.to_hex(),
                added_at_unix_ms: e.added_at_unix_ms,
                signature_hex: hex::encode(e.signature_bytes()),
            })
            .collect(),
    }
}

fn from_persisted(persisted: &PersistedCollection) -> anyhow::Result<Collection> {
    let id = CollectionId::from_string(&persisted.id)?;
    let invite_secret = InviteSecret::from_hex(&persisted.invite_secret_hex)?;
    let collaborators = persisted
        .collaborators
        .iter()
        .map(|c| -> anyhow::Result<Collaborator> {
            Ok(Collaborator::new(
                DeviceId::from_hex(&c.device_id_hex)?,
                c.display_name.clone(),
                if c.is_admin { Role::Admin } else { Role::Member },
                c.joined_at_unix_ms,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut manifest = Manifest::new();
    for e in &persisted.manifest {
        let info_hash_bytes: [u8; 20] = hex::decode(&e.info_hash_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("stored info hash is not 20 bytes"))?;
        let thumbnail_hash = e
            .thumbnail_hash_hex
            .as_ref()
            .map(|h| -> anyhow::Result<[u8; 32]> {
                hex::decode(h)?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("stored thumbnail hash is not 32 bytes"))
            })
            .transpose()?;
        let signature_bytes: [u8; 64] = hex::decode(&e.signature_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("stored signature is not 64 bytes"))?;
        manifest.add(ManifestEntry::from_signed_parts(
            InfoHash::from_bytes(info_hash_bytes),
            e.name.clone(),
            thumbnail_hash,
            DeviceId::from_hex(&e.added_by_hex)?,
            e.added_at_unix_ms,
            Signature::from_bytes(&signature_bytes),
        ));
    }
    Ok(Collection::from_parts(
        id,
        persisted.name.clone(),
        invite_secret,
        collaborators,
        manifest,
    ))
}

/// Loads every persisted collection, or an empty list if nothing's been
/// saved yet (a fresh install, or nothing created/joined so far).
pub(crate) fn load() -> anyhow::Result<Vec<Collection>> {
    let path = store_file();
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(Vec::new());
    };
    let persisted: PersistedStore =
        serde_json::from_slice(&bytes).context("parsing collections.json")?;
    persisted.collections.iter().map(from_persisted).collect()
}

/// Overwrites `collections.json` with the full current set.
pub(crate) fn save(collections: &[Collection]) -> anyhow::Result<()> {
    let path = store_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {parent:?}"))?;
    }
    let persisted = PersistedStore {
        collections: collections.iter().map(to_persisted).collect(),
    };
    let bytes = serde_json::to_vec_pretty(&persisted)?;
    std::fs::write(&path, bytes).with_context(|| format!("writing {path:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::DeviceIdentity;

    #[test]
    fn persisted_round_trip_preserves_id_collaborators_and_manifest() {
        let identity = DeviceIdentity::generate();
        let mut collection = Collection::new("Iceland 2024".into());
        collection.collaborators.push(Collaborator::new(
            identity.device_id(),
            "Maya".into(),
            Role::Admin,
            1_000,
        ));
        collection.add_manifest_entry(ManifestEntry::new_signed(
            InfoHash::from_bytes([3; 20]),
            "glacier.mp4".into(),
            None,
            &identity,
            2_000,
        ));

        let persisted = to_persisted(&collection);
        let reloaded = from_persisted(&persisted).unwrap();

        assert_eq!(reloaded.id, collection.id);
        assert_eq!(reloaded.name, collection.name);
        assert_eq!(
            reloaded.rendezvous_key().to_hex(),
            collection.rendezvous_key().to_hex()
        );
        assert_eq!(reloaded.collaborators.len(), 1);
        assert_eq!(reloaded.collaborators[0].display_name, "Maya");
        assert!(reloaded.collaborators[0].is_admin());
        assert_eq!(reloaded.manifest().len(), 1);
        assert!(reloaded.manifest().contains(&InfoHash::from_bytes([3; 20])));
    }

    #[test]
    fn persisted_manifest_entries_still_verify_after_round_trip() {
        let identity = DeviceIdentity::generate();
        let mut collection = Collection::new("Band Practice".into());
        collection.add_manifest_entry(ManifestEntry::new_signed(
            InfoHash::from_bytes([5; 20]),
            "take_4.wav".into(),
            None,
            &identity,
            3_000,
        ));

        let reloaded = from_persisted(&to_persisted(&collection)).unwrap();

        assert!(reloaded.manifest().entries().all(|e| e.verify()));
    }
}
