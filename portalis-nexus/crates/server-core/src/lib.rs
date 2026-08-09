//! Transport-independent Portalis Nexus server rules.
//!
//! Everything here is pure: no sockets, no database driver, no clock of its
//! own. Module layout:
//!
//! - [`negotiation`]: protocol version negotiation.
//! - [`handle`]: user handles, their rules, and allocation input.
//! - [`challenge`]: the one challenge a connection may sign.
//! - [`friendship`]: the friendship state machine over one canonical edge.
//! - [`friends`]: handle resolution, friend commands, and listing.
//! - [`presence`]: who is online, aggregated across a user's devices.
//! - [`ports`]: the storage, time, and randomness the domain depends on.
//! - [`identity`]: registration and device authentication.
//! - [`memory`]: in-memory ports for tests and local development.

mod challenge;
mod friends;
mod friendship;
mod handle;
mod identity;
mod memory;
mod negotiation;
mod ports;
mod presence;

pub use challenge::{ChallengeError, IssuedChallenge};
pub use friends::{COMMAND_ATTEMPTS, FriendError, FriendService, FriendSummary};
pub use friendship::{
    FriendshipEdge, FriendshipError, FriendshipRecord, Transition, apply as apply_friend_action,
};
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
pub use portalis_nexus_protocol::v1::{FriendAction, FriendshipState};
pub use ports::{
    Clock, DeviceId, DeviceKey, DeviceRecord, FriendRepository, IdentityRepository, RandomSource,
    RepositoryError, UserDirectory, UserId, UserRecord,
};
pub use presence::{ConnectionId, PresenceChange, PresenceRegistry};
