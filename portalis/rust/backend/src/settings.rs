//! Every setting the BitTorrent engine (`librqbit`) actually honours,
//! persisted and surfaced to Flutter.
//!
//! Two kinds, and the difference matters to the UI:
//!
//! - **Live**: the rate limits. `Session::ratelimits` is adjustable on a
//!   running session, so changing these takes effect immediately.
//! - **Restart-required**: everything else. librqbit takes them as
//!   `SessionOptions` at construction and offers no way to change them
//!   afterwards, so they're persisted here and read back by
//!   `torrent::session()` next time the session is built.
//!   [`set_engine_settings`] returns whether the change needs a restart, so
//!   the UI can say so rather than pretending it applied.
//!
//! Three `SessionOptions` fields are deliberately **not** exposed, because
//! they aren't settings in any user-facing sense:
//! `default_storage_factory` (a Rust trait object), `cancellation_token` and
//! `root_span` (engine internals wired up by this crate). `peer_id` is also
//! withheld: it's editable in principle, but a malformed one silently breaks
//! every peer handshake and there is no reason a user would want to.
//! `disable_upload` doesn't exist in our build — it's behind a librqbit
//! feature we don't enable (see Cargo.toml).
//!
//! Unlike `collab_store`, this module *is* FRB-scanned, which is why the
//! persisted form is [`EngineSettings`] itself — one all-`pub` struct that
//! is both the DTO and the on-disk format — rather than a private mirror
//! type that FRB's naive scan would try (and fail) to bridge.

use serde::{Deserialize, Serialize};

/// The engine's full configuration surface. `serde(default)` throughout so a
/// `settings.json` written by an older build still loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    // ---- Live: applied immediately to a running session ----
    /// Upload cap in bytes/sec across all torrents. `None` = unlimited.
    pub upload_limit_bps: Option<u32>,
    /// Download cap in bytes/sec across all torrents. `None` = unlimited.
    pub download_limit_bps: Option<u32>,

    // ---- Restart-required ----
    /// Range to pick the incoming-peer TCP port from. librqbit takes the
    /// first free port in it; with no range it binds *no* listener at all
    /// and nobody can download from this device.
    pub listen_port_start: u16,
    pub listen_port_end: u16,
    /// Ask the router (UPnP/IGD) to forward the listen port. A no-op on
    /// routers with UPnP disabled, and impossible while a VPN owns the
    /// default route (discovery follows that route instead of reaching the
    /// router).
    pub enable_upnp_port_forwarding: bool,
    /// `socks5://[user:pass@]host:port`. Routes peer traffic through a SOCKS5
    /// proxy.
    pub socks_proxy_url: Option<String>,

    /// Turn off the distributed hash table. Peers can then only be found via
    /// trackers or addresses handed over directly.
    pub disable_dht: bool,
    /// Stop reusing the stored DHT identity/port between runs. Off by
    /// default; note the stored port is why two instances of this app can't
    /// run at once.
    pub disable_dht_persistence: bool,

    /// Remember which torrents the session holds across restarts. Without it
    /// the engine starts empty every launch and silently stops seeding
    /// everything previously shared.
    pub persist_session: bool,
    /// Trust persisted piece state instead of re-hashing every file on
    /// launch, so collections resume seeding immediately.
    pub fastresume: bool,

    /// Buffer writes in memory up to roughly this many megabytes instead of
    /// writing straight through. `None` = write through.
    pub defer_writes_up_to_mb: Option<u32>,
    /// How many torrents may initialise concurrently. `None` = librqbit's
    /// default.
    pub concurrent_init_limit: Option<u32>,

    /// Per-peer connection timeouts, in seconds. `None` = librqbit's default.
    pub peer_connect_timeout_secs: Option<u32>,
    pub peer_read_write_timeout_secs: Option<u32>,
    pub peer_keep_alive_interval_secs: Option<u32>,

    /// URL of an IP blocklist to fetch and enforce.
    pub blocklist_url: Option<String>,
    /// Tracker URLs added to every torrent, on top of any it already carries.
    pub trackers: Vec<String>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            upload_limit_bps: None,
            download_limit_bps: None,
            // The conventional BitTorrent range. Must be non-empty: see the
            // field doc and `torrent::session`.
            listen_port_start: 6881,
            listen_port_end: 6999,
            enable_upnp_port_forwarding: true,
            socks_proxy_url: None,
            disable_dht: false,
            disable_dht_persistence: false,
            persist_session: true,
            fastresume: true,
            defer_writes_up_to_mb: None,
            concurrent_init_limit: None,
            peer_connect_timeout_secs: None,
            peer_read_write_timeout_secs: None,
            peer_keep_alive_interval_secs: None,
            blocklist_url: None,
            trackers: Vec::new(),
        }
    }
}

/// The settings the engine is configured with, loaded from disk (defaults on
/// first run).
pub fn engine_settings() -> anyhow::Result<EngineSettings> {
    native::load()
}

/// The built-in defaults, for a "reset" action in the UI.
pub fn default_engine_settings() -> EngineSettings {
    EngineSettings::default()
}

/// Persists `settings`, applying the live ones (rate limits) to the running
/// session straight away.
///
/// Returns `true` when a restart-required field changed *and* the session is
/// already running — i.e. the UI should tell the user the change lands next
/// launch. Returns `false` when everything asked for is already in effect.
pub async fn set_engine_settings(settings: EngineSettings) -> anyhow::Result<bool> {
    native::save_and_apply(settings).await
}

mod native {
        use std::sync::Mutex;

    
    use crate::log::clog;

    use super::EngineSettings;

    /// Cached so `torrent::session()` can read settings without touching the
    /// disk on every call.
    static CACHE: Mutex<Option<EngineSettings>> = Mutex::new(None);

    fn vault() -> crate::vault::Vault {
        crate::vault::Vault::named("settings.json")
    }

    pub(super) fn load() -> anyhow::Result<EngineSettings> {
        if let Some(cached) = CACHE.lock().unwrap().as_ref() {
            return Ok(cached.clone());
        }
        // A corrupt file must not brick the engine — an unusable settings
        // document falls back to defaults rather than refusing to start a
        // session. Absence does the same, and needs no complaint.
        let stored: Option<EngineSettings> = vault().read().unwrap_or_else(|e| {
            clog!("settings", "load: unusable ({e:#}), using defaults");
            None
        });
        let settings = stored.unwrap_or_default();
        *CACHE.lock().unwrap() = Some(settings.clone());
        Ok(settings)
    }

    fn save(settings: &EngineSettings) -> anyhow::Result<()> {
        vault().write(settings)
    }

    /// Fields librqbit only reads when the session is constructed.
    fn needs_restart(old: &EngineSettings, new: &EngineSettings) -> bool {
        // Compare everything *except* the live rate limits, by zeroing those
        // out — so adding a new restart-required field can't be forgotten
        // here the way an explicit field-by-field comparison would allow.
        let normalise = |s: &EngineSettings| EngineSettings {
            upload_limit_bps: None,
            download_limit_bps: None,
            ..s.clone()
        };
        normalise(old) != normalise(new)
    }

    pub(super) async fn save_and_apply(settings: EngineSettings) -> anyhow::Result<bool> {
        anyhow::ensure!(
            settings.listen_port_start <= settings.listen_port_end,
            "the listen port range starts after it ends"
        );
        anyhow::ensure!(
            settings.listen_port_start != 0,
            "0 is not a usable listen port"
        );

        let old = load()?;
        save(&settings)?;
        *CACHE.lock().unwrap() = Some(settings.clone());

        // Live half: applied now, whether or not anything else changed.
        crate::torrent::set_rate_limits(
            settings.upload_limit_bps,
            settings.download_limit_bps,
        )
        .await?;

        let restart_required = needs_restart(&old, &settings) && crate::torrent::session_started();
        clog!(
            "settings",
            "save_and_apply: applied; restart_required={restart_required}"
        );
        Ok(restart_required)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn changing_only_a_rate_limit_never_asks_for_a_restart() {
            let old = EngineSettings::default();
            let new = EngineSettings {
                upload_limit_bps: Some(500_000),
                download_limit_bps: Some(1_000_000),
                ..EngineSettings::default()
            };

            // These are the two librqbit adjusts on a live session.
            assert!(!needs_restart(&old, &new));
        }

        #[test]
        fn changing_a_construction_time_field_asks_for_a_restart() {
            let old = EngineSettings::default();

            for new in [
                EngineSettings { listen_port_start: 7000, ..EngineSettings::default() },
                EngineSettings { disable_dht: true, ..EngineSettings::default() },
                EngineSettings { persist_session: false, ..EngineSettings::default() },
                EngineSettings { fastresume: false, ..EngineSettings::default() },
                EngineSettings {
                    socks_proxy_url: Some("socks5://127.0.0.1:9050".into()),
                    ..EngineSettings::default()
                },
                EngineSettings {
                    trackers: vec!["udp://tracker.example:1337".into()],
                    ..EngineSettings::default()
                },
                EngineSettings { defer_writes_up_to_mb: Some(64), ..EngineSettings::default() },
                EngineSettings {
                    peer_connect_timeout_secs: Some(10),
                    ..EngineSettings::default()
                },
            ] {
                assert!(
                    needs_restart(&old, &new),
                    "expected a restart to be required for {new:?}"
                );
            }
        }

        #[test]
        fn settings_round_trip_through_json_and_tolerate_missing_fields() {
            let settings = EngineSettings {
                upload_limit_bps: Some(123),
                trackers: vec!["udp://a:1".into(), "udp://b:2".into()],
                ..EngineSettings::default()
            };

            let json = serde_json::to_vec(&settings).unwrap();
            assert_eq!(
                serde_json::from_slice::<EngineSettings>(&json).unwrap(),
                settings
            );

            // A file from an older build, missing every field this one knows
            // about, must still load rather than bricking the engine.
            let sparse: EngineSettings = serde_json::from_str("{}").unwrap();
            assert_eq!(sparse, EngineSettings::default());
        }
    }
}
