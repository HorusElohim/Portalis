//! Transport-independent Portalis Nexus server rules.
//!
//! Everything here is pure: no sockets, no database driver, no clock of its
//! own. Module layout:
//!
//! - [`negotiation`]: protocol version negotiation.
//! - [`handle`]: user handles, their rules, and allocation input.

mod handle;
mod negotiation;

pub use handle::{
    HANDLE_SEPARATOR, Handle, HandleError, discriminator_from_entropy, normalize_username,
    validate_discriminator, validate_username,
};
pub use negotiation::{NegotiationError, ProtocolPolicy};
