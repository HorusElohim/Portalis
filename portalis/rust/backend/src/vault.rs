//! One JSON document of state, read whole or written whole.
//!
//! Several modules once had their own copy of this — path, create the
//! directory, serialise, write, and rename a sibling into place. Identity was
//! the exception, written with a plain `fs::write`: the one file whose loss
//! cannot be recovered was the one without atomic replacement. That is what
//! duplicated persistence code costs.

use std::path::PathBuf;

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};

pub(crate) struct Vault {
    path: PathBuf,
}

impl Vault {
    /// A file by name inside the state directory — see [`crate::paths`].
    pub(crate) fn named(file: &str) -> Self {
        Self {
            path: crate::paths::state_dir().join(file),
        }
    }

    /// `None` when the file isn't there yet, which is a normal first run and
    /// not an error. A file that exists but won't parse *is* an error: silently
    /// treating corruption as absence would overwrite it on the next save.
    pub(crate) fn read<T: DeserializeOwned>(&self) -> anyhow::Result<Option<T>> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("reading {:?}", self.path));
            }
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .with_context(|| format!("parsing {:?}", self.path))
    }

    /// Written to a sibling and renamed over the target, so the document on
    /// disk is only ever the last complete one. A truncating write destroys
    /// what is there the instant it opens, before it can discover it has no
    /// room to finish — which is how this project lost a store to a full disk.
    pub(crate) fn write<T: Serialize>(&self, value: &T) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.path.parent().context("state dir has no parent")?)?;
        let staged = self.path.with_extension("tmp");
        std::fs::write(&staged, serde_json::to_vec_pretty(value)?)
            .with_context(|| format!("writing {staged:?}"))?;
        replace_file(&staged, &self.path).with_context(|| format!("replacing {:?}", self.path))
    }
}

#[cfg(not(windows))]
fn replace_file(staged: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(staged, target)
}

#[cfg(windows)]
fn replace_file(staged: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let staged: Vec<u16> = staged.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let succeeded = unsafe {
        MoveFileExW(
            staged.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

// The failed-write assertion needs Unix mode bits. The unreadable-path
// regression is portable and must keep running on Windows too.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_document_atomically_replaces_the_previous_one() {
        let _temp = crate::paths::redirect_to_temp();
        let vault = Vault::named("replace.json");

        vault.write(&1u32).unwrap();
        vault.write(&2u32).unwrap();

        assert_eq!(vault.read::<u32>().unwrap(), Some(2));
    }

    #[cfg(unix)]
    #[test]
    fn absent_reads_as_nothing_and_a_write_that_fails_keeps_the_last_one() {
        use std::os::unix::fs::PermissionsExt;
        let temp = crate::paths::redirect_to_temp();
        let vault = Vault::named("thing.json");

        assert_eq!(vault.read::<u32>().unwrap(), None);
        vault.write(&1u32).unwrap();
        assert_eq!(vault.read::<u32>().unwrap(), Some(1));

        // A directory nothing can create files in is a full disk's shape.
        let dir = temp.path("");
        std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o500)).unwrap();
        let refused = vault.write(&2u32);
        std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o700)).unwrap();

        assert!(refused.is_err());
        assert_eq!(vault.read::<u32>().unwrap(), Some(1));
    }

    #[test]
    fn an_unreadable_state_path_is_not_mistaken_for_a_first_run() {
        let temp = crate::paths::redirect_to_temp();
        std::fs::create_dir(temp.path("not-a-json-file.json")).unwrap();
        let vault = Vault::named("not-a-json-file.json");

        assert!(vault.read::<u32>().is_err());
    }
}
