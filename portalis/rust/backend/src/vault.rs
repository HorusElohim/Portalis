//! One JSON document of state, read whole or written whole.
//!
//! Four modules each had their own copy of this — path, create the directory,
//! serialise, write, and (three of the four) rename a sibling into place. The
//! fourth was `identity.json`, written with a plain `fs::write`: the one file
//! in the app whose loss cannot be recovered, carrying the key every signature
//! was ever made under, and the only one without the protection the other
//! three had. That is what four copies of a thing costs.

use std::path::PathBuf;

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};

pub(crate) struct Vault {
    path: PathBuf,
}

impl Vault {
    /// A file by name inside the state directory — see [`crate::paths`].
    pub(crate) fn named(file: &str) -> Self {
        Self { path: crate::paths::state_dir().join(file) }
    }

    /// `None` when the file isn't there yet, which is a normal first run and
    /// not an error. A file that exists but won't parse *is* an error: silently
    /// treating corruption as absence would overwrite it on the next save.
    pub(crate) fn read<T: DeserializeOwned>(&self) -> anyhow::Result<Option<T>> {
        let Ok(bytes) = std::fs::read(&self.path) else {
            return Ok(None);
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
        std::fs::rename(&staged, &self.path)
            .with_context(|| format!("replacing {:?}", self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
