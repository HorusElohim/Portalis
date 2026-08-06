//! The one authoritative place where collection bytes live.
//!
//! A location is deliberately not a Flutter path string. Today the concrete
//! filesystem implementation serves desktop and the Portalis Files library on
//! iOS. Android MediaStore URIs will add a native random-access implementation
//! here; they must never be converted to a cache path merely to fit a path API.

use std::path::{Path, PathBuf};

/// A canonical location whose bytes may be hashed, transferred, previewed,
/// and seeded. Each collection item has one such location.
#[derive(Debug, Clone)]
pub(crate) enum ContentLocation {
    Filesystem(PathBuf),
}

impl ContentLocation {
    /// Converts the current Flutter bridge representation to a native
    /// location. `content://` is rejected intentionally until the Android
    /// adapter can retain its URI permission and provide random access.
    pub(crate) fn from_source_path(source: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !source.starts_with("content://"),
            "Android media URIs need Portalis' native no-copy storage adapter"
        );
        anyhow::ensure!(
            !source.trim().is_empty(),
            "a source location cannot be empty"
        );
        Ok(Self::Filesystem(PathBuf::from(source)))
    }

    pub(crate) fn filesystem_path(&self) -> &Path {
        match self {
            Self::Filesystem(path) => path,
        }
    }

    pub(crate) fn metadata(&self) -> anyhow::Result<std::fs::Metadata> {
        std::fs::metadata(self.filesystem_path())
            .map_err(|error| anyhow::anyhow!("cannot read source {:?}: {error}", self.filesystem_path()))
    }
}

#[cfg(test)]
mod tests {
    use super::ContentLocation;

    #[test]
    fn rejects_a_uri_instead_of_turning_it_into_a_cache_path() {
        assert!(ContentLocation::from_source_path("content://media/external/images/1").is_err());
    }

    #[test]
    fn accepts_a_filesystem_location() {
        let location = ContentLocation::from_source_path("C:/Media/photo.jpg").unwrap();
        assert_eq!(location.filesystem_path().to_string_lossy(), "C:/Media/photo.jpg");
    }
}
