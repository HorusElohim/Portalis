#![cfg_attr(not(frb_expand), allow(unexpected_cfgs))]
mod api; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
use flutter_rust_bridge::frb;

// Keep web simple by making this a synchronous, non-threaded function.
// FRB will generate a sync binding that avoids web worker/threadpool usage.
#[frb(sync)]
pub fn get_version() -> String {
    "0.0.1".to_string()
}
