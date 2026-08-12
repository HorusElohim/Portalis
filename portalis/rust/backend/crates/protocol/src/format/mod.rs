//! Canonical byte formats.
//!
//! Every layout here is a contract rather than a serialisation choice: the
//! bytes are hashed, signed, or authenticated, so a change to any of them
//! changes what every other participant computes. That is why they are
//! hand-written and length-prefixed rather than produced by a library whose
//! encoding is not ours to pin (`SPEC.md` D10).
//!
//! They live in `protocol` because both sides need them — the service
//! verifies signatures on write, and clients verify everything they read.
//! Sealing and opening a content key stay in the client: the service must
//! never gain the ability to decrypt.
//!
//! - [`aead`]: the one place bytes are encrypted, and the envelope they share.
//! - [`devicelog`]: a person's devices, signed and append-only.
//! - [`manifest`]: the canonical list of a revision's entries, and its hash.
//! - [`revision`]: a collection's history, as a chain of signed revisions.
//! - [`sealed`]: that manifest, encrypted under a collection's content key.
//! - [`entry`]: one entry's `.torrent`, encrypted under the same key.

pub mod aead;
pub mod devicelog;
pub mod entry;
pub mod manifest;
pub mod revision;
pub mod sealed;
