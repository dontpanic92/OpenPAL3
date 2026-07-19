//! Versioned project manifest: the durable record of every asset
//! add/replace change an asset project tracks, keyed by the package it
//! targets plus the entry path within that package.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::atomic::{atomic_write, read_file, unix_now};
use crate::error::{AssetProjectError, Result};
use crate::hash::ContentHash;

/// Highest `ProjectManifest::version` this build knows how to read.
/// Bump this (and add migration logic) when the on-disk schema gains a
/// backward-incompatible change.
pub const PROJECT_MANIFEST_VERSION: u32 = 1;

/// Validates a package-relative path: forward slashes only, not
/// absolute, and no `.`/`..` traversal component. Unlike a "normalize
/// away the weird bits" approach, invalid input is rejected outright —
/// a caller passing `../../etc/passwd` or `/etc/passwd` almost
/// certainly has a bug (or is trying to escape the target package),
/// and silently coercing it into some other, "safe" path would hide
/// that instead of surfacing it.
fn validate_relative_path(path: &str) -> Result<String> {
    let normalized = path.replace('\\', "/");

    if normalized.is_empty() {
        return Err(AssetProjectError::InvalidPath {
            path: path.to_string(),
            reason: "path is empty".to_string(),
        });
    }
    if normalized.starts_with('/') {
        return Err(AssetProjectError::InvalidPath {
            path: path.to_string(),
            reason: "absolute paths are not allowed".to_string(),
        });
    }
    // A `C:`-style drive prefix would still be treated as a single odd
    // path component below and rejected for containing `:`... but
    // let's be explicit for Windows-style absolute paths too.
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Err(AssetProjectError::InvalidPath {
            path: path.to_string(),
            reason: "absolute (drive-letter) paths are not allowed".to_string(),
        });
    }

    for component in normalized.split('/') {
        if component.is_empty() {
            return Err(AssetProjectError::InvalidPath {
                path: path.to_string(),
                reason: "empty path component (e.g. from \"//\")".to_string(),
            });
        }
        if component == "." || component == ".." {
            return Err(AssetProjectError::InvalidPath {
                path: path.to_string(),
                reason: format!("path traversal component {component:?} is not allowed"),
            });
        }
    }

    Ok(normalized)
}

/// Shared implementation for the package-relative path newtypes below:
/// same validation, same (de)serialization as a plain string, same
/// `Display`/accessors, but distinct Rust types so `target_package` and
/// `package_internal_path` can't be accidentally swapped at a call
/// site.
macro_rules! relative_path_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(path: impl AsRef<str>) -> Result<Self> {
                Ok(Self(validate_relative_path(path.as_ref())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::convert::TryFrom<&str> for $name {
            type Error = AssetProjectError;
            fn try_from(value: &str) -> Result<Self> {
                Self::new(value)
            }
        }

        impl std::convert::TryFrom<String> for $name {
            type Error = AssetProjectError;
            fn try_from(value: String) -> Result<Self> {
                Self::new(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

relative_path_newtype!(
    /// Relative path to the target `.cpk`/`.ypk` package a change
    /// applies to (e.g. `scene.cpk`, `basedata/basedata.cpk`).
    /// Validated at construction time: no absolute paths, no `.`/`..`
    /// traversal components.
    TargetPackage
);

relative_path_newtype!(
    /// Path of an asset entry *within* its target package (e.g.
    /// `m01/xxx.dff`). Validated at construction time: no absolute
    /// paths, no `.`/`..` traversal components.
    PackagePath
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetChangeKind {
    /// A (target_package, package_internal_path) pair that does not
    /// exist in the base install.
    Add,
    /// A (target_package, package_internal_path) pair that replaces an
    /// existing entry.
    Replace,
}

/// Where an asset's payload originally came from, before conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSource {
    /// Path to the raw/original asset (e.g. a `.png` or `.fbx`) in the
    /// project's source tree, relative to the project root.
    pub original_path: PathBuf,
    /// Content hash of the raw source file at import time, so the
    /// editor can tell whether the source changed since the last
    /// conversion without re-hashing large binary trees eagerly.
    pub source_hash: Option<ContentHash>,
}

/// Records how a payload was produced from its source, so a patch (or
/// a bug report) carries enough information to reproduce it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionMetadata {
    pub tool: String,
    pub tool_version: String,
    /// Free-form tool parameters (e.g. compression level, mip count).
    /// Kept as a string map so individual tools can add fields without
    /// a manifest schema bump.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// Unix seconds when the conversion was performed.
    pub converted_at: u64,
}

/// A reference to converted payload bytes held in a
/// [`crate::payload_store::PayloadStore`] — the payload itself is
/// looked up by content hash, so `AssetChange` never has to carry (or
/// get out of sync with) a loose filesystem path to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadRef {
    pub content_hash: ContentHash,
    pub size: u64,
}

impl PayloadRef {
    pub fn of(data: &[u8]) -> Self {
        Self {
            content_hash: ContentHash::of(data),
            size: data.len() as u64,
        }
    }
}

/// A single tracked change: either introducing a new package entry
/// (`Add`) or overwriting an existing one (`Replace`) at
/// `(target_package, package_internal_path)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetChange {
    pub kind: AssetChangeKind,
    /// Which `.cpk`/`.ypk` package this change applies to, relative to
    /// the project's `base_asset_root` (e.g. `scene.cpk`).
    pub target_package: TargetPackage,
    /// Path of the entry within `target_package` (e.g. `m01/xxx.dff`).
    pub package_internal_path: PackagePath,
    /// Reference to the converted payload bytes in a `PayloadStore`.
    pub payload: PayloadRef,
    /// For `Replace` changes: hash of the entry being overwritten in
    /// the base package, so an installer can refuse to apply this
    /// change if the target package's existing entry doesn't match
    /// what the patch was built against (optimistic-concurrency check
    /// against an unexpectedly-drifted install). `None` for `Add`
    /// changes, and may be `None` for `Replace` when the base entry's
    /// hash wasn't captured at authoring time.
    pub base_entry_hash: Option<ContentHash>,
    pub source: Option<AssetSource>,
    pub conversion: Option<ConversionMetadata>,
}

impl AssetChange {
    /// Builds an `AssetChange` from converted payload bytes, computing
    /// the `PayloadRef` for you so callers can't accidentally desync a
    /// change record from its actual payload.
    #[allow(clippy::too_many_arguments)]
    pub fn from_payload(
        kind: AssetChangeKind,
        target_package: TargetPackage,
        package_internal_path: PackagePath,
        payload: &[u8],
        base_entry_hash: Option<ContentHash>,
        source: Option<AssetSource>,
        conversion: Option<ConversionMetadata>,
    ) -> Self {
        Self {
            kind,
            target_package,
            package_internal_path,
            payload: PayloadRef::of(payload),
            base_entry_hash,
            source,
            conversion,
        }
    }

    pub fn is_add(&self) -> bool {
        matches!(self.kind, AssetChangeKind::Add)
    }

    pub fn is_replace(&self) -> bool {
        matches!(self.kind, AssetChangeKind::Replace)
    }

    /// The key this change is stored/looked-up under in a
    /// [`ProjectManifest`]: `(target_package, package_internal_path)`.
    pub fn key(&self) -> AssetChangeKey {
        AssetChangeKey {
            target_package: self.target_package.clone(),
            package_internal_path: self.package_internal_path.clone(),
        }
    }
}

/// Uniquely identifies one tracked change: which package it targets,
/// plus its path within that package. A single `package_internal_path`
/// may exist identically in two different target packages, so both
/// components are required to disambiguate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetChangeKey {
    pub target_package: TargetPackage,
    pub package_internal_path: PackagePath,
}

impl AssetChangeKey {
    pub fn new(target_package: TargetPackage, package_internal_path: PackagePath) -> Self {
        Self {
            target_package,
            package_internal_path,
        }
    }
}

/// The versioned, persisted record of every asset change an asset
/// project tracks. Changes are keyed by [`AssetChangeKey`] so
/// re-recording a change to the same `(target_package,
/// package_internal_path)` replaces the previous entry rather than
/// accumulating duplicates.
///
/// `(De)serialize` are implemented manually rather than derived: JSON
/// object keys must be strings, but `AssetChangeKey` is a struct, so
/// `changes` is (de)serialized as a plain `Vec<AssetChange>` on the
/// wire (with the `BTreeMap` rebuilt from each change's own `key()` on
/// deserialize) while still giving callers key-based lookup/dedup in
/// memory.
#[derive(Debug, Clone)]
pub struct ProjectManifest {
    pub version: u32,
    pub project_id: Uuid,
    pub name: String,
    /// Which game/config key this project targets, matching
    /// `yaobow::shared::GameType::config_key` (e.g. `"pal3"`,
    /// `"pal4"`), so a `.ybpatch` built from this project can be
    /// checked against the right game before installing.
    pub target_game: String,
    /// Root of the base asset tree (the unpacked/mounted `.cpk` set)
    /// this project's changes are layered on top of.
    pub base_asset_root: PathBuf,
    pub created_at: u64,
    pub updated_at: u64,
    changes: BTreeMap<AssetChangeKey, AssetChange>,
}

/// Wire representation of [`ProjectManifest`]: identical fields, but
/// `changes` is a plain list instead of a map (see the note on
/// `ProjectManifest` for why).
#[derive(Serialize, Deserialize)]
struct ProjectManifestWire {
    version: u32,
    project_id: Uuid,
    name: String,
    target_game: String,
    base_asset_root: PathBuf,
    created_at: u64,
    updated_at: u64,
    changes: Vec<AssetChange>,
}

impl Serialize for ProjectManifest {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let wire = ProjectManifestWire {
            version: self.version,
            project_id: self.project_id,
            name: self.name.clone(),
            target_game: self.target_game.clone(),
            base_asset_root: self.base_asset_root.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            changes: self.changes.values().cloned().collect(),
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProjectManifest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let wire = ProjectManifestWire::deserialize(deserializer)?;
        let changes = wire
            .changes
            .into_iter()
            .map(|change| (change.key(), change))
            .collect();
        Ok(Self {
            version: wire.version,
            project_id: wire.project_id,
            name: wire.name,
            target_game: wire.target_game,
            base_asset_root: wire.base_asset_root,
            created_at: wire.created_at,
            updated_at: wire.updated_at,
            changes,
        })
    }
}

impl ProjectManifest {
    pub fn new(
        name: impl Into<String>,
        target_game: impl Into<String>,
        base_asset_root: impl Into<PathBuf>,
    ) -> Self {
        let now = unix_now();
        Self {
            version: PROJECT_MANIFEST_VERSION,
            project_id: Uuid::new_v4(),
            name: name.into(),
            target_game: target_game.into(),
            base_asset_root: base_asset_root.into(),
            created_at: now,
            updated_at: now,
            changes: BTreeMap::new(),
        }
    }

    /// Inserts or overwrites the change recorded for
    /// `change.key()`, returning the previous entry (if any) and
    /// bumping `updated_at`.
    pub fn upsert_change(&mut self, change: AssetChange) -> Option<AssetChange> {
        self.updated_at = unix_now();
        self.changes.insert(change.key(), change)
    }

    pub fn remove_change(&mut self, key: &AssetChangeKey) -> Option<AssetChange> {
        let removed = self.changes.remove(key);
        if removed.is_some() {
            self.updated_at = unix_now();
        }
        removed
    }

    pub fn get_change(&self, key: &AssetChangeKey) -> Option<&AssetChange> {
        self.changes.get(key)
    }

    pub fn changes(&self) -> impl Iterator<Item = &AssetChange> {
        self.changes.values()
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(|e| AssetProjectError::json("<manifest>", e))
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|e| AssetProjectError::json("<manifest>", e))?;
        manifest.check_version()?;
        Ok(manifest)
    }

    fn check_version(&self) -> Result<()> {
        if self.version > PROJECT_MANIFEST_VERSION {
            return Err(AssetProjectError::UnsupportedManifestVersion {
                found: self.version,
                supported: PROJECT_MANIFEST_VERSION,
            });
        }
        Ok(())
    }

    /// Atomically persists the manifest as pretty-printed JSON.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| AssetProjectError::json(path, e))?;
        atomic_write(path, &bytes)
    }

    /// Loads a manifest previously written by [`ProjectManifest::save`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = read_file(path)?;
        let manifest: Self =
            serde_json::from_slice(&bytes).map_err(|e| AssetProjectError::json(path, e))?;
        manifest.check_version()?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!("{}-{}-{}", name, std::process::id(), unix_now()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_change(package_internal_path: &str) -> AssetChange {
        AssetChange::from_payload(
            AssetChangeKind::Add,
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new(package_internal_path).unwrap(),
            b"payload bytes",
            None,
            Some(AssetSource {
                original_path: PathBuf::from(format!("source/{package_internal_path}")),
                source_hash: Some(ContentHash::of(b"raw source bytes")),
            }),
            Some(ConversionMetadata {
                tool: "dds_convert".into(),
                tool_version: "1.0.0".into(),
                params: BTreeMap::new(),
                converted_at: unix_now(),
            }),
        )
    }

    #[test]
    fn package_path_accepts_normal_relative_paths() {
        assert_eq!(
            PackagePath::new("a\\b\\c.txt").unwrap().as_str(),
            "a/b/c.txt"
        );
        assert_eq!(PackagePath::new("a/b.txt").unwrap().as_str(), "a/b.txt");
    }

    #[test]
    fn package_path_rejects_absolute_paths() {
        let err = PackagePath::new("/a/b.txt").unwrap_err();
        assert!(matches!(err, AssetProjectError::InvalidPath { .. }));

        let err = PackagePath::new("C:\\a\\b.txt").unwrap_err();
        assert!(matches!(err, AssetProjectError::InvalidPath { .. }));
    }

    #[test]
    fn package_path_rejects_dot_and_dotdot_traversal() {
        assert!(matches!(
            PackagePath::new("./a/b.txt").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
        assert!(matches!(
            PackagePath::new("../a/b.txt").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
        assert!(matches!(
            PackagePath::new("a/../b.txt").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
        assert!(matches!(
            PackagePath::new("a/./b.txt").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
    }

    #[test]
    fn package_path_rejects_empty_and_double_slash() {
        assert!(matches!(
            PackagePath::new("").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
        assert!(matches!(
            PackagePath::new("a//b.txt").unwrap_err(),
            AssetProjectError::InvalidPath { .. }
        ));
    }

    #[test]
    fn target_package_uses_the_same_validation() {
        assert!(TargetPackage::new("scene.cpk").is_ok());
        assert!(TargetPackage::new("/etc/passwd").is_err());
        assert!(TargetPackage::new("../../etc/passwd").is_err());
    }

    #[test]
    fn upsert_replaces_and_bumps_updated_at() {
        let mut manifest = ProjectManifest::new("test-project", "pal3", "/base/assets");
        let created = manifest.created_at;

        assert!(manifest.upsert_change(sample_change("a.txt")).is_none());
        assert_eq!(manifest.len(), 1);

        let mut replacement = sample_change("a.txt");
        replacement.kind = AssetChangeKind::Replace;
        replacement.base_entry_hash = Some(ContentHash::of(b"old a.txt contents"));
        let previous = manifest.upsert_change(replacement.clone());
        assert!(previous.is_some());
        assert_eq!(manifest.len(), 1);

        let key = AssetChangeKey::new(
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new("a.txt").unwrap(),
        );
        let stored = manifest.get_change(&key).unwrap();
        assert_eq!(stored.kind, AssetChangeKind::Replace);
        assert!(stored.base_entry_hash.is_some());
        assert_eq!(manifest.created_at, created);
    }

    #[test]
    fn remove_change_removes_and_reports_absence() {
        let mut manifest = ProjectManifest::new("test-project", "pal3", "/base/assets");
        manifest.upsert_change(sample_change("a.txt"));

        let key = AssetChangeKey::new(
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new("a.txt").unwrap(),
        );
        assert!(manifest.remove_change(&key).is_some());
        assert!(manifest.is_empty());
        assert!(manifest.remove_change(&key).is_none());
    }

    #[test]
    fn same_internal_path_in_different_packages_are_distinct_changes() {
        let mut manifest = ProjectManifest::new("test-project", "pal3", "/base/assets");

        let mut change_a = sample_change("shared/name.txt");
        change_a.target_package = TargetPackage::new("scene.cpk").unwrap();
        let mut change_b = sample_change("shared/name.txt");
        change_b.target_package = TargetPackage::new("basedata.cpk").unwrap();

        manifest.upsert_change(change_a);
        manifest.upsert_change(change_b);
        assert_eq!(manifest.len(), 2);
    }

    #[test]
    fn json_roundtrip_preserves_changes_and_project_fields() {
        let mut manifest = ProjectManifest::new("roundtrip-project", "pal4", "/base/pal4-assets");
        manifest.upsert_change(sample_change("a.txt"));
        manifest.upsert_change(sample_change("b/c.txt"));

        let json = manifest.to_json().unwrap();
        let restored = ProjectManifest::from_json(&json).unwrap();

        assert_eq!(restored.name, manifest.name);
        assert_eq!(restored.target_game, "pal4");
        assert_eq!(restored.base_asset_root, PathBuf::from("/base/pal4-assets"));
        assert_eq!(restored.project_id, manifest.project_id);
        assert_eq!(restored.len(), 2);

        let key = AssetChangeKey::new(
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new("a.txt").unwrap(),
        );
        assert_eq!(restored.get_change(&key), manifest.get_change(&key));
    }

    #[test]
    fn rejects_manifest_from_a_newer_schema_version() {
        let mut manifest = ProjectManifest::new("future-project", "pal3", "/base");
        manifest.version = PROJECT_MANIFEST_VERSION + 1;
        let json = manifest.to_json().unwrap();

        let err = ProjectManifest::from_json(&json).unwrap_err();
        assert!(matches!(
            err,
            AssetProjectError::UnsupportedManifestVersion { .. }
        ));
    }

    #[test]
    fn deserialization_rejects_path_traversal() {
        assert!(serde_json::from_str::<PackagePath>(r#""../evil.mv3""#).is_err());
        assert!(serde_json::from_str::<TargetPackage>(r#""/absolute.cpk""#).is_err());
    }

    #[test]
    fn save_and_load_round_trip_on_disk() {
        let dir = scratch_dir("project-manifest");
        let path = dir.join("project.json");

        let mut manifest = ProjectManifest::new("disk-project", "pal3", &dir);
        manifest.upsert_change(sample_change("a.txt"));
        manifest.save(&path).unwrap();

        let loaded = ProjectManifest::load(&path).unwrap();
        assert_eq!(loaded.project_id, manifest.project_id);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.base_asset_root, dir);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
