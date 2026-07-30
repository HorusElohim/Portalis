//! Minimal diagnostic logging for the collab/sync subsystem — a single
//! `clog!("tag", "message {}", value)` call site instead of scattered
//! `eprintln!`s, so every line gets the same `[millis-since-epoch] [tag]`
//! prefix (useful for lining up events across two devices' logs by eye)
//! and there's one place to change *how* this is emitted later (e.g.
//! writing to a file for release builds) without touching every call
//! site. Deliberately not the `tracing`/`log` crates: this app has no
//! subscriber wired up anywhere, and for a debug aid meant to be read
//! directly off `flutter run`'s console, plain `eprintln!` is the only
//! thing guaranteed to show up with zero extra setup.
//!
//! Not FRB-scanned (not in `tool/frb_build.sh`'s `--rust-input`) — it has
//! no bridged items, but keeping it out is consistent with `domain`/
//! `collab_store` and avoids the naive-scan problem entirely by
//! construction.

pub(crate) fn log(tag: &str, args: std::fmt::Arguments) {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    eprintln!("[{millis}] [{tag}] {args}");
}

/// `clog!("collab", "join: name={name:?}")` — tag first, then
/// `format!`-style args.
macro_rules! clog {
    ($tag:expr, $($arg:tt)*) => {
        $crate::log::log($tag, format_args!($($arg)*))
    };
}
pub(crate) use clog;
