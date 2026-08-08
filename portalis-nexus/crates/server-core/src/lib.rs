//! Transport-independent Portalis Nexus server rules.
//!
//! Everything here is pure: no sockets, no database driver, no clock of its
//! own. Module layout:
//!
//! - [`negotiation`]: protocol version negotiation.
//! - [`handle`]: user handles, their rules, and allocation input.
//! - [`challenge`]: the one challenge a connection may sign.
//! - [`ports`]: the storage, time, and randomness the domain depends on.
//! - [`identity`]: registration and device authentication.
//! - [`memory`]: in-memory ports for tests and local development.

mod challenge;
mod handle;
mod identity;
mod memory;
mod negotiation;
mod ports;

pub use challenge::{ChallengeError, IssuedChallenge};
pub use handle::{
    HANDLE_SEPARATOR, Handle, HandleError, discriminator_from_entropy, normalize_username,
    validate_discriminator, validate_username,
};
pub use identity::{
    AuthenticationRequest, HANDLE_ALLOCATION_ATTEMPTS, Identity, IdentityError, IdentityService,
    RegistrationRequest,
};
pub use memory::{FixedClock, InMemoryIdentities, ScriptedRandom};
pub use negotiation::{NegotiationError, ProtocolPolicy};
pub use ports::{
    Clock, DeviceId, DeviceKey, DeviceRecord, IdentityRepository, RandomSource, RepositoryError,
    UserId, UserRecord,
};
