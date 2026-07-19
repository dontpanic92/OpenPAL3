//! Error types shared by every module in this crate.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatcherError {
    #[error(transparent)]
    AssetProject(#[from] asset_project::AssetProjectError),

    #[error("failed to rebuild package {path}: {source}")]
    CpkRebuild {
        path: PathBuf,
        #[source]
        source: packfs::cpk::CpkRebuildError,
    },

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

    #[error("failed to parse TOML at {path}: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("validation failed: {0}")]
    ValidationFailed(String),

    #[error("target package {0:?} was not found under the game root")]
    PackageNotFound(String),

    #[error("package {0:?} is not writable: {1}")]
    PackageNotWritable(String, String),

    #[error(
        "package fingerprint mismatch for {target_package:?}: expected {expected}, found {actual}"
    )]
    FingerprintMismatch {
        target_package: String,
        expected: String,
        actual: String,
    },

    #[error(
        "base entry hash mismatch for {target_package:?}/{package_internal_path:?}: expected {expected}, found {actual}"
    )]
    BaseEntryHashMismatch {
        target_package: String,
        package_internal_path: String,
        expected: String,
        actual: String,
    },

    #[error("patch targets game {patch_game:?}, but the selected root is {root_game:?}")]
    GameMismatch {
        patch_game: String,
        root_game: String,
    },

    #[error("patch {0} is not recorded as applied in the installation journal")]
    PatchNotApplied(Uuid),

    #[error("no backups found for patch {0}; cannot roll back")]
    NoBackupsForPatch(Uuid),

    #[error(
        "cannot roll back patch {patch_id}: newer applied patch(es) {blocking_patch_ids:?} also \
         touch package(s) {overlapping_packages:?}; roll those back first (in journal order) \
         before rolling back {patch_id}"
    )]
    RollbackBlockedByNewerPatch {
        patch_id: Uuid,
        blocking_patch_ids: Vec<Uuid>,
        overlapping_packages: Vec<String>,
    },

    #[error("backup for package {0:?} is corrupt (hash mismatch); refusing to roll back")]
    CorruptBackup(String),

    #[error("managed patch {patch_id} already exists with different content")]
    ManagedPatchIdCollision { patch_id: Uuid },

    #[error("managed patch {0} was not found in the selected game root")]
    ManagedPatchNotFound(Uuid),

    #[error(
        "unsupported mod-manager state version {found}; highest supported version is {supported}"
    )]
    UnsupportedManagerStateVersion { found: u32, supported: u32 },

    #[error("transaction aborted by injected fault at {0}")]
    InjectedFault(String),

    #[error("{0}")]
    Other(String),
}

use uuid::Uuid;

pub type Result<T> = std::result::Result<T, PatcherError>;

impl PatcherError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }

    /// Currently unused: [`crate::config`]'s TOML read is
    /// best-effort (`Option`-returning, never surfaces a parse error),
    /// so nothing constructs `PatcherError::Toml` today. Kept for a
    /// future stricter config-loading path rather than removed, since
    /// the `Toml` variant itself is part of this crate's public error
    /// surface.
    #[allow(dead_code)]
    pub(crate) fn toml(path: impl Into<PathBuf>, source: toml::de::Error) -> Self {
        Self::Toml {
            path: path.into(),
            source,
        }
    }
}
