#![cfg_attr(not(frb_expand), allow(unexpected_cfgs))]
mod api; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
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
