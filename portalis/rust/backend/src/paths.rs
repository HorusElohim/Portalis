//! Where Portalis keeps the state it must not lose.
//!
//! Persistent files such as `identity.json`, `collections.json`,
//! `settings.json`, `sync_peers.json`, and `imports.json` once rebuilt this
//! path independently. Naming it once lets a test point all of them somewhere
//! disposable, and until it was named there was no way to test any writer
//! against a real file: the one test of the atomic write *mirrored* `save`'s
//! strategy against a temp path rather than calling `save`, so a regression to
//! a truncating `fs::write` would have gone unnoticed by the test written to
//! catch precisely that.

use std::path::PathBuf;

// Per-thread, because the test harness gives each test its own thread — so
// redirection is isolated for free, and there is nothing to serialise.
// Production never sets it; the whole mechanism compiles away.
#[cfg(test)]
thread_local! {
    static OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(not(test))]
pub(crate) fn state_dir() -> PathBuf {
    platform_state_dir()
}

#[cfg(test)]
pub(crate) fn state_dir() -> PathBuf {
    OVERRIDE
        .with(|dir| dir.borrow().clone())
        .unwrap_or_else(platform_state_dir)
}

/// `~/Library/Application Support/Portalis`, `%APPDATA%\Portalis`, and so on —
/// falling back through the data directory to a temp one, so a platform with
/// neither still runs rather than refusing to start.
fn platform_state_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Portalis")
}

// Redirected tests run one at a time. The paths are per-thread, but the state
// *behind* them is not — `collab_store::STORE` and the settings cache are
// process-wide, so two redirected tests would see each other's writes. Taking
// this here means no future test has to remember.
#[cfg(test)]
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Redirects every persisted file to a fresh empty directory for the rest of
/// this test, and removes it afterwards.
#[cfg(test)]
pub(crate) fn redirect_to_temp() -> TempState {
    // A panicking test poisons the lock; the next one still deserves to run.
    let guard = SERIAL.lock().unwrap_or_else(|held| held.into_inner());
    let dir = std::env::temp_dir().join(format!("portalis-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("creating a temp state dir");
    OVERRIDE.with(|slot| *slot.borrow_mut() = Some(dir.clone()));
    TempState { dir, _guard: guard }
}

/// Undoes [`redirect_to_temp`] when it goes out of scope, so a failing
/// assertion cannot leave the next test looking at someone else's state.
#[cfg(test)]
pub(crate) struct TempState {
    dir: PathBuf,
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl TempState {
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

#[cfg(test)]
impl Drop for TempState {
    fn drop(&mut self) {
        OVERRIDE.with(|slot| *slot.borrow_mut() = None);
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirection_is_scoped_and_cleans_up_after_itself() {
        let real = platform_state_dir();
        let path = {
            let temp = redirect_to_temp();
            assert_ne!(state_dir(), real);
            assert_eq!(state_dir(), temp.dir);
            temp.dir.clone()
        };

        // Out of scope: the override is gone and so is the directory. A test
        // that fails mid-way must not leave either behind.
        assert_eq!(state_dir(), real);
        assert!(!path.exists());
    }
}
