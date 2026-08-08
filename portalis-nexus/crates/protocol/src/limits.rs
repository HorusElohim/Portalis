//! Protocol constants shared by every crate in the workspace.
//!
//! These are the wire limits from `SPEC.md` section 8. Changing one is a
//! protocol decision, not an implementation detail.

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;
pub const MESSAGE_ID_BYTES: usize = 16;
pub const CONNECTION_ID_BYTES: usize = 16;
pub const CHALLENGE_BYTES: usize = 32;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PENDING_REQUESTS: usize = 128;
pub const MAX_OUTBOUND_QUEUE: usize = 256;
pub const WEBSOCKET_SUBPROTOCOL: &str = "portalis.protobuf.v1";
