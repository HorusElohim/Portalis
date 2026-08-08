//! The portable Portalis Nexus client.
//!
//! This crate builds for macOS, iOS, Android, and Linux. It has no `MongoDB`,
//! `Axum`, or server-core dependency.
//!
//! Module layout:
//!
//! - [`error`]: deterministic protocol failures.
//! - [`protocol`]: client-side message construction and validation.
//! - [`pending`]: the bounded request/response correlation registry.
//! - [`reconnect`]: bounded exponential reconnect scheduling.
//! - [`signer`]: how a caller proves it owns a device key.
//! - [`config`]: tuning for one supervised connection.
//! - [`transport`]: the socket actor those rules drive.

mod config;
mod error;
mod pending;
mod protocol;
mod reconnect;
mod signer;
mod transport;

pub use config::{ClientConfig, DEFAULT_REQUEST_TIMEOUT};
pub use error::ClientError;
pub use pending::PendingRequests;
pub use protocol::{ClientProtocol, validate_authenticated, validate_hello, validate_pong};
pub use reconnect::{ReconnectPolicy, ReconnectPolicyError};
pub use signer::DeviceSigner;
pub use transport::{NexusClient, TransportError, authority_of};
