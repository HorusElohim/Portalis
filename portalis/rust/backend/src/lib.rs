#![cfg_attr(not(frb_expand), allow(unexpected_cfgs))]
mod api; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
mod domain;
pub mod bridge;
// Unconditional on every target — see torrent.rs's module doc for why
// (flutter_rust_bridge's generated glue references this module regardless
// of any #[cfg] on its own declaration). librqbit itself is still gated to
// non-wasm32 in Cargo.toml; torrent.rs's internals mirror that.
pub mod torrent;
pub mod device;
pub mod collab;
