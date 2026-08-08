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
mod signing;
mod validate;

pub use frame::{FrameError, decode_frame, encode_frame, validate_frame_size};
pub use ids::{
    UUID_V7_ENTROPY_BYTES, derive_device_id, format_id, new_challenge, new_message_id, user_id_from,
};
pub use limits::{
    CHALLENGE_BYTES, CHALLENGE_LIFETIME_MS, CONNECTION_ID_BYTES, CURRENT_PROTOCOL_VERSION,
    DEVICE_ID_BYTES, DEVICE_KEY_BYTES, DISCRIMINATOR_CHARS, MAX_FRAME_BYTES, MAX_OUTBOUND_QUEUE,
    MAX_PENDING_REQUESTS, MAX_USERNAME_CHARS, MESSAGE_ID_BYTES, MIN_USERNAME_CHARS,
    SIGNATURE_BYTES, USER_ID_BYTES, WEBSOCKET_SUBPROTOCOL,
};
pub use signing::{
    AUTHENTICATION_CONTEXT, REGISTRATION_CONTEXT, SessionBinding, SignatureError,
    authentication_payload, registration_payload, verify_signature,
};
pub use validate::{ServerHelloValidationError, ValidationError, validate_server_hello};
