//! Minimal diagnostic logging for the collab/sync subsystem — a single
//! `clog!("tag", "message {}", value)` call site instead of scattered
//! `eprintln!`s, so every line gets the same `[millis-since-epoch] [tag]`
//! prefix (useful for lining up events across two devices' logs by eye)
//! and there's one place to change *how* this is emitted later (e.g.
//! writing to a file for release builds) without touching every call
//! site. Deliberately not the `tracing`/`log` crates: this app has no
//! subscriber wired up anywhere, and for a debug aid meant to be read
//! directly off `flutter run`'s console, writing straight to stderr is the
//! only thing guaranteed to show up with zero extra setup — but see
//! [`log`] for why it must not be `eprintln!`.
//!
//! Not FRB-scanned (not in `tool/frb_build.sh`'s `--rust-input`) — it has
//! no bridged items, but keeping it out is consistent with `domain`/
//! `collab_store` and avoids the naive-scan problem entirely by
//! construction.

/// Writes one diagnostic line, and **never fails**.
///
/// Deliberately not `eprintln!`: that macro panics if the write fails, so a
/// stderr that has gone away — a closed terminal, a pipe whose reader
/// exited, a GUI launch with no console attached — turns the next log call
/// into a panic that propagates across the FFI boundary as
/// `PanicException(failed printing to stderr: Broken pipe (os error 32))`
/// and takes down whatever backend call happened to be logging. Observed
/// exactly that on macOS. A diagnostic aid must never be able to break the
/// thing it is diagnosing, so the write error is dropped: losing a log line
/// is always preferable to losing the operation.
pub(crate) fn log(tag: &str, args: std::fmt::Arguments) {
    use std::io::Write;

    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "[{millis}] [{tag}] {args}");
}

/// `clog!("collab", "join: name={name:?}")` — tag first, then
/// `format!`-style args.
macro_rules! clog {
    ($tag:expr, $($arg:tt)*) => {
        $crate::log::log($tag, format_args!($($arg)*))
    };
}
pub(crate) use clog;
