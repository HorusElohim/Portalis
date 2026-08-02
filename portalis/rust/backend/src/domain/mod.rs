//! Domain layer: pure logic, no I/O, no `librqbit`, no FRB types. Fully
//! unit-testable in isolation. See `rust/backend/README.md` for the big
//! picture and the UML class diagram this module implements.
//!
//! Everything here is `pub(crate)`, not `pub` — these types are never meant
//! to cross the FFI boundary directly (see the README's "Flutter boundary
//! API" section). `flutter_rust_bridge`'s codegen scans for the bare `pub`
//! keyword on individual items, not true Rust visibility resolution, so it
//! would otherwise try to bridge these regardless of `domain` itself being
//! a private module of `lib.rs`.

pub(crate) mod collaborator;
pub(crate) mod collection;
pub(crate) mod identity;
pub(crate) mod invite;
pub(crate) mod manifest;
