//! Small top-level FRB-facing functions that don't belong to any specific
//! bridged module. Exists so `flutter_rust_bridge_codegen`'s `--rust-input`
//! can list explicit module paths (`crate::bridge,crate::torrent`, see
//! `tool/frb_build.sh`) instead of the bare `crate` wildcard — which walks
//! every `mod` declaration in the crate regardless of visibility and would
//! sweep up `domain`'s internal types (see `rust/backend/README.md`).

use flutter_rust_bridge::frb;

// Keep web simple by making this a synchronous, non-threaded function.
// FRB will generate a sync binding that avoids web worker/threadpool usage.
#[frb(sync)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_version_matches_crate_metadata() {
        assert_eq!(get_version(), env!("CARGO_PKG_VERSION"));
    }
}
