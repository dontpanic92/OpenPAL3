//! Fingerprinting helpers shared by validation and patch authoring.
//!
//! [`asset_project::patch::PackageFingerprint`] deliberately leaves the
//! derivation of `base_hash` up to the caller (see its doc comment).
//! This installer always uses a whole-file content hash of the
//! physical package (`.cpk`) — simple, deterministic, and doesn't
//! require understanding the package's internal layout to compute.
//! `.yapatch` authors targeting this installer must use the same
//! convention when building [`asset_project::patch::PackageFingerprint`]s.

use std::fs;
use std::path::Path;

use asset_project::ContentHash;
use packfs::cpk::CpkArchive;

use crate::error::{PatcherError, Result};

/// Whole-file content hash of the package at `path`. This is the
/// fingerprint scheme this installer expects `.yapatch` authors to use
/// for `PackageFingerprint::base_hash`.
pub fn package_fingerprint(path: impl AsRef<Path>) -> Result<ContentHash> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|e| PatcherError::io(path, e))?;
    Ok(ContentHash::of(&bytes))
}

/// Hash of one entry's *decompressed* content inside a (non-PAL4)
/// `.cpk` package, matching `AssetChange::base_entry_hash`'s
/// derivation: the same [`ContentHash`] algorithm applied to the raw
/// bytes a reader would get back from opening that entry.
///
/// `internal_path` uses the on-disk `.cpk` convention (backslash or
/// forward-slash separated, case-insensitive).
pub fn base_entry_hash(package_path: impl AsRef<Path>, internal_path: &str) -> Result<ContentHash> {
    use std::io::Read;

    let package_path = package_path.as_ref();
    let file = fs::File::open(package_path).map_err(|e| PatcherError::io(package_path, e))?;
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(file))).map_err(|e| {
        PatcherError::Io {
            path: package_path.to_path_buf(),
            source: e,
        }
    })?;

    let normalized = internal_path.replace('/', "\\");
    let mut entry = archive
        .open_str(&normalized)
        .map_err(|e| PatcherError::Io {
            path: package_path.join(&normalized),
            source: e,
        })?;

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| PatcherError::io(package_path.join(&normalized), e))?;

    Ok(ContentHash::of(&buf))
}

/// Whether `internal_path` exists in the (non-PAL4) `.cpk` at
/// `package_path`, without reading its full content.
pub fn entry_exists(package_path: impl AsRef<Path>, internal_path: &str) -> Result<bool> {
    let package_path = package_path.as_ref();
    let file = fs::File::open(package_path).map_err(|e| PatcherError::io(package_path, e))?;
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(file))).map_err(|e| {
        PatcherError::Io {
            path: package_path.to_path_buf(),
            source: e,
        }
    })?;

    let normalized = internal_path.replace('/', "\\");
    Ok(archive.open_str(&normalized).is_ok())
}
