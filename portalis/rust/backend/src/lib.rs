#![cfg_attr(not(frb_expand), allow(unexpected_cfgs))]
mod api; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
mod domain;
mod log;
// Private and NOT part of tool/frb_build.sh's --rust-input, same reason as
// `domain` — see collab_store.rs's own module doc.
mod collab_store;
// Real sockets — native targets only, like librqbit.
mod collab_sync;
pub mod bridge;
// Unconditional on every target — see torrent.rs's module doc for why
// (flutter_rust_bridge's generated glue references this module regardless
// of any #[cfg] on its own declaration). librqbit itself is still gated to
// non-wasm32 in Cargo.toml; torrent.rs's internals mirror that.
pub mod torrent;
pub mod device;
/// The one collection model the app renders — see its module doc. Replaces
/// the former `collab` module, which was one half of a pair that was never
/// joined.
pub mod collections;
