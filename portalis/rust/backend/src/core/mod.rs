//! The application core: what components say to each other, and who owns them.
//!
//! Everything written for v3 lives in a directory under `src/`, and the flat
//! modules beside it are the ones being replaced. That is not a filing
//! convention — the coverage gate reads it, so new work is held to 100% from
//! its first line while a module scheduled for deletion is not.
//!
//! - [`events`]: the bus, and the facts that travel on it (`SPEC.md` §11).
//! - [`supervisor`]: task ownership, startup order, bounded shutdown.

pub mod events;
pub mod supervisor;
