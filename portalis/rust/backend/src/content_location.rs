//! The one authoritative place where collection bytes live.
//!
//! A location is deliberately not a Flutter path string. Today the concrete
//! filesystem implementation serves desktop and the Portalis Files library on
//! iOS. Android MediaStore URIs will add a native random-access implementation
//! here; they must never be converted to a cache path merely to fit a path API.

use std::path::{Path, PathBuf};

#[cfg(target_os = "ios")]
use std::ffi::CString;

/// A canonical location whose bytes may be hashed, transferred, previewed,
/// and seeded. Each collection item has one such location.
#[derive(Debug, Clone)]
pub(crate) enum ContentLocation {
    Filesystem(PathBuf),
    #[cfg(target_os = "ios")]
    PhotoAsset(String),
}

impl ContentLocation {
    /// Converts the Flutter bridge representation to a native location.
    pub(crate) fn from_source_path(source: &str) -> anyhow::Result<Self> {
        #[cfg(target_os = "ios")]
        if let Some(identifier) = source.strip_prefix("phasset://") {
            anyhow::ensure!(
                !identifier.is_empty(),
                "a Photos identifier cannot be empty"
            );
            return Ok(Self::PhotoAsset(identifier.into()));
        }
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
            #[cfg(target_os = "ios")]
            Self::PhotoAsset(_) => panic!("a Photos asset has no filesystem path"),
        }
    }

    pub(crate) fn length(&self, known_length: Option<u64>) -> anyhow::Result<u64> {
        #[cfg(not(target_os = "ios"))]
        let _ = known_length;
        match self {
            Self::Filesystem(path) => std::fs::metadata(path)
                .map(|metadata| metadata.len())
                .map_err(|error| anyhow::anyhow!("cannot read source {path:?}: {error}")),
            #[cfg(target_os = "ios")]
            Self::PhotoAsset(identifier) => {
                anyhow::ensure!(
                    photo_asset_available(identifier),
                    "Photos asset is no longer available"
                );
                let measured_length = photo_asset_length(identifier)?;
                if known_length.is_some_and(|length| length != measured_length) {
                    crate::log::clog!("torrent", "Photos asset length changed from picker metadata; using native resource length");
                }
                Ok(measured_length)
            }
        }
    }

    pub(crate) fn requires_native_storage(&self) -> bool {
        match self {
            Self::Filesystem(_) => false,
            #[cfg(target_os = "ios")]
            Self::PhotoAsset(_) => true,
        }
    }

    pub(crate) fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> anyhow::Result<()> {
        match self {
            Self::Filesystem(path) => {
                #[cfg(target_family = "unix")]
                {
                    use std::os::unix::fs::FileExt;
                    std::fs::File::open(path)?.read_exact_at(buffer, offset)?;
                    Ok(())
                }
                #[cfg(not(target_family = "unix"))]
                {
                    use std::io::{Read, Seek, SeekFrom};
                    let mut file = std::fs::File::open(path)?;
                    file.seek(SeekFrom::Start(offset))?;
                    file.read_exact(buffer)?;
                    Ok(())
                }
            }
            #[cfg(target_os = "ios")]
            Self::PhotoAsset(identifier) => photo_asset_read(identifier, offset, buffer),
        }
    }
}

#[cfg(target_os = "ios")]
fn photo_asset_available(identifier: &str) -> bool {
    let Ok(identifier) = CString::new(identifier) else {
        return false;
    };
    unsafe { portalis_photo_asset_available(identifier.as_ptr()) }
}

#[cfg(target_os = "ios")]
fn photo_asset_read(identifier: &str, offset: u64, buffer: &mut [u8]) -> anyhow::Result<()> {
    let identifier = CString::new(identifier)?;
    let result = unsafe {
        portalis_photo_asset_read(
            identifier.as_ptr(),
            offset,
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    anyhow::ensure!(
        result == 0,
        "PhotoKit could not read the requested asset range ({result})"
    );
    Ok(())
}

#[cfg(target_os = "ios")]
fn photo_asset_length(identifier: &str) -> anyhow::Result<u64> {
    let identifier = CString::new(identifier)?;
    let length = unsafe { portalis_photo_asset_length(identifier.as_ptr()) };
    anyhow::ensure!(
        length > 0,
        "PhotoKit could not determine the selected asset length ({length})"
    );
    Ok(length as u64)
}

#[cfg(target_os = "ios")]
extern "C" {
    fn portalis_photo_asset_available(identifier: *const std::ffi::c_char) -> bool;
    fn portalis_photo_asset_length(identifier: *const std::ffi::c_char) -> i64;
    fn portalis_photo_asset_read(
        identifier: *const std::ffi::c_char,
        offset: u64,
        buffer: *mut u8,
        length: usize,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::ContentLocation;

    #[test]
    fn rejects_an_android_uri_instead_of_turning_it_into_a_cache_path() {
        assert!(ContentLocation::from_source_path("content://media/external/images/1").is_err());
    }

    #[test]
    fn accepts_a_filesystem_location() {
        let location = ContentLocation::from_source_path("C:/Media/photo.jpg").unwrap();
        assert_eq!(
            location.filesystem_path().to_string_lossy(),
            "C:/Media/photo.jpg"
        );
    }
}
