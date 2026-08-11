//! The portable Portalis Nexus client.
//!
//! This crate builds for macOS, iOS, Android, and Linux. It has no `MongoDB`,
//! `Axum`, or server-core dependency.
//!
//! Module layout:
//!
//! - [`error`]: deterministic protocol failures.
//! - [`manifest`]: the canonical media manifest and its content root.
//! - [`capsule`]: sealing that manifest into bytes Nexus cannot read.
//! - [`protocol`]: client-side message construction and validation.
//! - [`pending`]: the bounded request/response correlation registry.
//! - [`reconnect`]: bounded exponential reconnect scheduling.
//! - [`signer`]: how a caller proves it owns a device key.
//! - [`config`]: tuning for one supervised connection.
//! - [`transport`]: the socket actor those rules drive.

mod candidates;
mod capsule;
mod config;
mod error;
mod manifest;
mod pending;
mod protocol;
mod reconnect;
mod signer;
mod transport;

pub use candidates::{CandidateSource, PeerCandidate, merge_candidates};
pub use capsule::{
    CAPSULE_VERSION, CapsuleContext, CapsuleError, SHARE_KEY_BYTES, ShareKey, open as open_capsule,
    seal as seal_capsule,
};
pub use config::{ClientConfig, DEFAULT_REQUEST_TIMEOUT};
pub use error::ClientError;
pub use manifest::{
    ENTRY_VERSION, INFO_HASH_BYTES, MAX_ENTRIES, MAX_ENTRY_NAME_BYTES, Manifest, ManifestEntry,
    ManifestError, SnapshotId, THUMBNAIL_HASH_BYTES,
};
pub use pending::PendingRequests;
pub use protocol::{
    ClientProtocol, KeyEnvelopePage, validate_authenticated, validate_device_linked,
    validate_friend_event, validate_friend_list, validate_hello, validate_key_envelope_put,
    validate_key_envelopes, validate_peer_announced, validate_peer_lookup, validate_peer_withdrawn,
    validate_pong, validate_reply, validate_resolved, validate_share_access_granted,
    validate_share_access_revoked, validate_share_fetch, validate_share_handoff,
    validate_share_list, validate_share_published,
};
pub use reconnect::{ReconnectPolicy, ReconnectPolicyError};
pub use signer::DeviceSigner;
pub use transport::{NexusClient, TransportError, authority_of};
