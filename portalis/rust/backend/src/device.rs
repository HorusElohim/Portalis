//! Device identity, exposed to Flutter for the first time (previously only
//! `domain::identity` existed, unused by anything). Backs the User screen:
//! a real Ed25519 keypair generated once and persisted locally, with an
//! editable nickname — no accounts, no servers, matching the design in
//! `rust/backend/README.md`.
//!
//! Unconditional signatures for the same reason as `torrent.rs`: FRB's
//! generated glue references `crate::device::*` regardless of any `#[cfg]`
//! on this module's own declaration.

#[derive(Debug, Clone)]
pub struct DeviceIdentityInfo {
    pub device_id: String,
    pub nickname: String,
}

/// Loads the persisted identity, generating and saving one on first call.
pub fn device_identity() -> anyhow::Result<DeviceIdentityInfo> {
    native::device_identity()
}

/// Renames this device's identity (display name only — the keypair itself
/// never changes).
pub fn set_nickname(nickname: String) -> anyhow::Result<DeviceIdentityInfo> {
    native::set_nickname(nickname)
}

/// The actual signing keypair — for other backend modules that need to
/// sign something themselves (e.g. `collections.rs` signing manifest
/// entries),
/// never exposed to Flutter directly the way `device_identity()`'s DTO is.
pub(crate) fn current_identity() -> anyhow::Result<crate::domain::identity::DeviceIdentity> {
    current_nexus_identity().map(crate::nexus::NexusIdentity::into_signing_identity)
}

/// Both private keys used by the online Nexus workflow. Kept below the bridge
/// so callers can sign and open envelopes without exposing key material.
pub(crate) fn current_nexus_identity() -> anyhow::Result<crate::nexus::NexusIdentity> {
    native::load_or_create().map(|(identity, _nickname)| identity)
}

mod native {
    use std::sync::Mutex;

    use anyhow::Context;
    use serde::{Deserialize, Serialize};

    use crate::domain::identity::DeviceIdentity;
    use crate::nexus::NexusIdentity;

    use super::DeviceIdentityInfo;

    #[derive(Serialize, Deserialize)]
    struct PersistedIdentity {
        /// Hex-encoded 32-byte Ed25519 signing key.
        secret_key_hex: String,
        /// Hex-encoded 32-byte X25519 secret. Older installations do not
        /// carry it; loading one generates and durably writes it before the
        /// identity can be used for Nexus registration.
        #[serde(default)]
        encryption_secret_key_hex: Option<String>,
        nickname: String,
    }

    static CACHE: Mutex<Option<DeviceIdentityInfo>> = Mutex::new(None);

    #[cfg(test)]
    pub(super) fn forget_cache_for_test() {
        *CACHE.lock().unwrap() = None;
    }

    fn vault() -> crate::vault::Vault {
        crate::vault::Vault::named("identity.json")
    }

    pub(super) fn load_or_create() -> anyhow::Result<(NexusIdentity, String)> {
        if let Some(persisted) = vault().read::<PersistedIdentity>()? {
            return restore(persisted);
        }

        let identity = NexusIdentity::generate(DeviceIdentity::generate());
        let nickname = "Me".to_string();
        crate::log::clog!(
            "device",
            "no identity yet, generated one, device_id={}…",
            &identity.signing_identity().device_id().to_hex()[..8]
        );
        save(&identity, &nickname)?;
        Ok((identity, nickname))
    }

    fn restore(persisted: PersistedIdentity) -> anyhow::Result<(NexusIdentity, String)> {
        let key: [u8; 32] = hex::decode(&persisted.secret_key_hex)
            .context("decoding stored secret key")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("stored secret key is not 32 bytes"))?;
        let signing = DeviceIdentity::from_bytes(&key);
        let Some(encryption_secret_key_hex) = persisted.encryption_secret_key_hex else {
            let identity = NexusIdentity::generate(signing);
            save(&identity, &persisted.nickname)?;
            crate::log::clog!(
                "device",
                "added the missing Nexus encryption key to the existing identity"
            );
            return Ok((identity, persisted.nickname));
        };
        let encryption: [u8; 32] = hex::decode(encryption_secret_key_hex)
            .context("decoding stored encryption secret key")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("stored encryption secret key is not 32 bytes"))?;
        let identity = NexusIdentity::from_parts(signing, encryption);
        Ok((identity, persisted.nickname))
    }

    fn save(identity: &NexusIdentity, nickname: &str) -> anyhow::Result<()> {
        vault().write(&PersistedIdentity {
            secret_key_hex: hex::encode(identity.signing_identity().to_bytes()),
            encryption_secret_key_hex: Some(hex::encode(identity.encryption_secret())),
            nickname: nickname.to_string(),
        })
    }

    pub(super) fn device_identity() -> anyhow::Result<DeviceIdentityInfo> {
        let mut cache = CACHE.lock().unwrap();
        if let Some(info) = cache.as_ref() {
            return Ok(info.clone());
        }
        let (identity, nickname) = load_or_create()?;
        let info = DeviceIdentityInfo {
            // The existing bridge contract still names the raw Ed25519 key.
            // Migrating collection collaborator IDs is a separate,
            // version-aware persistence change; Nexus itself uses the
            // derived ID exposed by `NexusIdentity`.
            device_id: identity.signing_identity().device_id().to_hex(),
            nickname,
        };
        *cache = Some(info.clone());
        Ok(info)
    }

    pub(super) fn set_nickname(nickname: String) -> anyhow::Result<DeviceIdentityInfo> {
        let (identity, _old_nickname) = load_or_create()?;
        save(&identity, &nickname)?;
        // Collaborator records hold a *copy* of the name taken when the
        // collection was created or joined, so renaming the identity alone
        // left every existing collection showing the old one — and kept
        // sending it to peers, since the collaborator list is what sync
        // exchanges. Non-fatal: the rename itself has already been saved,
        // and a collection whose record didn't update is a stale label, not
        // a broken collection.
        if let Err(e) =
            crate::collab_store::rename_device(&identity.signing_identity().device_id(), &nickname)
        {
            crate::log::clog!(
                "device",
                "set_nickname: renamed the identity but couldn't update collections ({e:#})"
            );
        }
        let info = DeviceIdentityInfo {
            device_id: identity.signing_identity().device_id().to_hex(),
            nickname,
        };
        *CACHE.lock().unwrap() = Some(info.clone());
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_client::DeviceSigner;
    use serde::Serialize;

    use super::*;

    #[derive(Serialize)]
    struct LegacyPersistedIdentity {
        secret_key_hex: String,
        nickname: String,
    }

    /// Losing this file loses the identity every signature was made under —
    /// there is no recovery, so the round trip is worth asserting on disk.
    #[test]
    fn the_identity_survives_a_reload() {
        let _temp = crate::paths::redirect_to_temp();
        let first = native::load_or_create()
            .unwrap()
            .0
            .signing_identity()
            .device_id();

        native::forget_cache_for_test();

        assert_eq!(
            native::load_or_create()
                .unwrap()
                .0
                .signing_identity()
                .device_id(),
            first
        );
    }

    #[test]
    fn an_existing_identity_gets_one_durable_encryption_key() {
        let _temp = crate::paths::redirect_to_temp();
        let signing_secret = [13_u8; 32];
        crate::vault::Vault::named("identity.json")
            .write(&LegacyPersistedIdentity {
                secret_key_hex: hex::encode(signing_secret),
                nickname: "Maya".into(),
            })
            .unwrap();

        let (migrated, nickname) = native::load_or_create().unwrap();
        let encryption_public_key = migrated.encryption_public_key();
        assert_eq!(migrated.signing_identity().to_bytes(), signing_secret);
        assert_eq!(nickname, "Maya");

        let (reloaded, _) = native::load_or_create().unwrap();
        assert_eq!(reloaded.encryption_public_key(), encryption_public_key);

        let stored: serde_json::Value = crate::vault::Vault::named("identity.json")
            .read()
            .unwrap()
            .unwrap();
        assert_eq!(
            stored["encryption_secret_key_hex"].as_str().unwrap().len(),
            64
        );
    }
}
