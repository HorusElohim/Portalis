//! Protocol constants shared by every crate in the workspace.
//!
//! These are the wire limits from `SPEC.md` section 8. Changing one is a
//! protocol decision, not an implementation detail.

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const MESSAGE_ID_BYTES: usize = 16;
pub const CONNECTION_ID_BYTES: usize = 16;
pub const CHALLENGE_BYTES: usize = 32;
pub const USER_ID_BYTES: usize = 16;
pub const DEVICE_ID_BYTES: usize = 32;
pub const DEVICE_KEY_BYTES: usize = 32;
pub const ENCRYPTION_KEY_BYTES: usize = 32;
/// Client-generated and opaque to Nexus: M2.5 scopes key envelopes to it
/// without the ownership, revision, or membership record M4 adds later.
pub const SHARE_ID_BYTES: usize = 16;
pub const SIGNATURE_BYTES: usize = 64;
pub const MIN_USERNAME_CHARS: usize = 3;
pub const MAX_USERNAME_CHARS: usize = 24;
pub const DISCRIMINATOR_CHARS: usize = 5;
/// Nanoseconds in one millisecond, for the few places that still need
/// milliseconds: `UUIDv7` timestamps and anything speaking to a system that
/// defines its own unit.
pub const NANOS_PER_MILLI: u64 = 1_000_000;
/// How long a `ServerHello` challenge may be used to sign: one minute.
pub const CHALLENGE_LIFETIME_NS: u64 = 60 * 1_000 * NANOS_PER_MILLI;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PENDING_REQUESTS: usize = 128;
pub const MAX_OUTBOUND_QUEUE: usize = 256;
/// A sealed share key is tiny; this leaves room for future key metadata while
/// preventing opaque envelope storage from becoming a general file upload.
pub const MAX_KEY_ENVELOPE_CIPHERTEXT_BYTES: usize = 4 * 1024;
/// Key envelopes are fetched in deterministic pages so a device with many
/// shares cannot exceed the frame or memory budget in one response.
pub const MAX_KEY_ENVELOPES_PER_PAGE: usize = 128;
pub const WEBSOCKET_SUBPROTOCOL: &str = "portalis.protobuf.v1";
