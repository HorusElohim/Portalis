//! Domain layer: pure logic, no I/O, no `librqbit`, no FRB types. Fully
//! unit-testable in isolation. See `rust/backend/README.md` for the big
//! picture and the UML class diagram this module implements.

pub mod collaborator;
pub mod collection;
pub mod identity;
pub mod invite;
pub mod manifest;
pub mod media_item;
