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
    native::load_or_create().map(|(identity, _nickname)| identity)
}

mod native {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use anyhow::Context;
    use serde::{Deserialize, Serialize};

    use crate::domain::identity::DeviceIdentity;

    use super::DeviceIdentityInfo;

    #[derive(Serialize, Deserialize)]
    struct PersistedIdentity {
        /// Hex-encoded 32-byte Ed25519 signing key.
        secret_key_hex: String,
        nickname: String,
    }

    static CACHE: Mutex<Option<DeviceIdentityInfo>> = Mutex::new(None);

    fn identity_file() -> PathBuf {
        let base = dirs::config_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir);
        base.join("Portalis").join("identity.json")
    }

    pub(super) fn load_or_create() -> anyhow::Result<(DeviceIdentity, String)> {
        let path = identity_file();
        if let Ok(bytes) = std::fs::read(&path) {
            let persisted: PersistedIdentity =
                serde_json::from_slice(&bytes).context("parsing identity.json")?;
            let key_bytes: [u8; 32] = hex::decode(&persisted.secret_key_hex)
                .context("decoding stored secret key")?
                .try_into()
                .map_err(|_| anyhow::anyhow!("stored secret key is not 32 bytes"))?;
            let identity = DeviceIdentity::from_bytes(&key_bytes);
            crate::log::clog!(
                "device",
                "load_or_create: loaded existing identity from {path:?}, device_id={}…",
                &identity.device_id().to_hex()[..8]
            );
            return Ok((identity, persisted.nickname));
        }

        let identity = DeviceIdentity::generate();
        let nickname = "Me".to_string();
        crate::log::clog!(
            "device",
            "load_or_create: no identity file at {path:?}, generated a new one, device_id={}…",
            &identity.device_id().to_hex()[..8]
        );
        save(&identity, &nickname)?;
        Ok((identity, nickname))
    }

    fn save(identity: &DeviceIdentity, nickname: &str) -> anyhow::Result<()> {
        let path = identity_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {parent:?}"))?;
        }
        let persisted = PersistedIdentity {
            secret_key_hex: hex::encode(identity.to_bytes()),
            nickname: nickname.to_string(),
        };
        let bytes = serde_json::to_vec_pretty(&persisted)?;
        std::fs::write(&path, bytes).with_context(|| format!("writing {path:?}"))?;
        Ok(())
    }

    pub(super) fn device_identity() -> anyhow::Result<DeviceIdentityInfo> {
        let mut cache = CACHE.lock().unwrap();
        if let Some(info) = cache.as_ref() {
            return Ok(info.clone());
        }
        let (identity, nickname) = load_or_create()?;
        let info = DeviceIdentityInfo {
            device_id: identity.device_id().to_hex(),
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
        if let Err(e) = crate::collab_store::rename_device(&identity.device_id(), &nickname) {
            crate::log::clog!(
                "device",
                "set_nickname: renamed the identity but couldn't update collections ({e:#})"
            );
        }
        let info = DeviceIdentityInfo {
            device_id: identity.device_id().to_hex(),
            nickname,
        };
        *CACHE.lock().unwrap() = Some(info.clone());
        Ok(info)
    }
}

