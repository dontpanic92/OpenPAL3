//! Error types shared by every module in this crate.
//!
//! Every I/O-carrying variant records the path it was operating on so
//! that editor/patcher callers can surface a useful message without
//! having to thread path context through call sites themselves.

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetProjectError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to (de)serialize JSON at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "manifest schema version {found} is newer than the highest version this build supports ({supported})"
    )]
    UnsupportedManifestVersion { found: u32, supported: u32 },

    #[error("content hash mismatch for {path}: expected {expected}, found {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("invalid content hash {0:?}")]
    InvalidHash(String),

    /// Raised for any package-relative path (`TargetPackage`,
    /// `PackagePath`) that is absolute or contains a `.`/`..`
    /// traversal component. Such paths are rejected outright rather
    /// than silently normalized, since a project/patch author most
    /// likely made a mistake, and quietly reinterpreting `../../etc`
    /// as a harmless relative path would be an easy way to smuggle a
    /// path-traversal payload into an installed game tree.
    #[error("invalid path {path:?}: {reason}")]
    InvalidPath { path: String, reason: String },

    #[error("entry {0:?} not found in .yapatch archive")]
    MissingPatchEntry(String),

    #[error("asset change {0:?} not found in project manifest")]
    UnknownChange(String),

    #[error("duplicate package path {0:?} in project manifest")]
    DuplicatePackagePath(String),

    /// Wraps failures from the underlying `radiance::asset::ypk`
    /// reader/writer, which report errors as `anyhow::Error`. The
    /// message is captured as a `String` rather than keeping the
    /// `anyhow::Error` itself, since `anyhow::Error` does not implement
    /// `std::error::Error` and therefore can't be used as a `#[source]`.
    #[error("ypk archive error: {0}")]
    Ypk(String),

    #[error("patch {0} is already recorded in the installation journal")]
    DuplicateJournalEntry(String),
}

impl AssetProjectError {
    pub(crate) fn io(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub(crate) fn json(path: impl AsRef<Path>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, AssetProjectError>;
