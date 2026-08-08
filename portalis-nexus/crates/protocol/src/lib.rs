//! The authoritative Portalis Nexus protocol contract.
//!
//! This crate owns the generated protobuf types, the wire limits from
//! `SPEC.md`, and the validation every peer applies before dispatch. It has no
//! sockets, database drivers, or platform adapters.

#[allow(clippy::doc_markdown, clippy::must_use_candidate)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/portalis.protocol.v1.rs"));
}

mod frame;
mod ids;
mod limits;
mod validate;

pub use frame::{FrameError, decode_frame, encode_frame, validate_frame_size};
pub use ids::{format_id, new_challenge, new_message_id};
pub use limits::{
    CHALLENGE_BYTES, CONNECTION_ID_BYTES, CURRENT_PROTOCOL_VERSION, MAX_FRAME_BYTES,
    MAX_OUTBOUND_QUEUE, MAX_PENDING_REQUESTS, MESSAGE_ID_BYTES, WEBSOCKET_SUBPROTOCOL,
};
pub use validate::{ServerHelloValidationError, ValidationError, validate_server_hello};
