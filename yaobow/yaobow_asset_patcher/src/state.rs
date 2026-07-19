//! Per-package transaction state: fine-grained progress tracking for
//! one patch install, layered *alongside*
//! `asset_project::journal::InstallationJournal` (which only records
//! coarse `Pending`/`Applied`/`Failed`/`RolledBack` for the whole
//! patch). This is what lets a crash mid-swap-phase be recovered
//! precisely: for each touched package we know whether it was only
//! backed up, had its temp `.cpk` built, was already swapped into
//! place, or has since been committed/rolled back.
//!
//! Persisted with the exact same atomic-write discipline as
//! `asset_project`'s own manifest/journal (`asset_project::atomic::atomic_write`):
//! the whole state is one JSON document, written to a sibling temp
//! file and renamed into place, so a crash can never leave a
//! half-written state file.

use std::path::{Path, PathBuf};

use asset_project::atomic::{atomic_write, read_file};
use asset_project::hash::ContentHash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{PatcherError, Result};

pub const TRANSACTION_STATE_FILE_NAME: &str = "state.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageStage {
    /// Recorded when this package's entry is first added to the
    /// state, before its backup has been written.
    Planned,
    /// Pre-patch backup copied to the patch-specific backup
    /// directory; `backup_hash` is filled in at this point.
    BackedUp,
    /// Sibling temp `.cpk` built (from the *original*, not-yet-swapped
    /// package) and verified against every change's declared payload
    /// hash.
    TempBuilt,
    /// Persisted *before* `crate::replace::replace_file` is called for
    /// this package, durably recording intent to swap before any
    /// on-disk change happens. This closes the crash window that would
    /// otherwise exist between `replace_file` completing (including
    /// its own best-effort marker cleanup) and the caller persisting
    /// [`PackageStage::Swapped`] afterwards: without this stage, a
    /// crash in that window leaves a live file that already holds the
    /// swapped-in bytes but a recorded stage that never advanced past
    /// `TempBuilt`, which startup recovery (which only ever restores
    /// packages already marked `Swapped`) would then silently miss.
    /// `crate::replace::reconcile_stale_replacements` treats a package
    /// found in this stage as needing disambiguation from disk state
    /// (marker presence, and whether the sibling temp file was
    /// consumed) rather than assuming either outcome.
    SwapStarted,
    /// The temp `.cpk` has been atomically renamed over the live
    /// package. From this point on, restoring requires the backup.
    Swapped,
    /// Every touched package reached `Swapped` and the whole
    /// transaction was recorded `Applied` in the installation journal.
    Committed,
    /// This package's live file has been restored from its backup
    /// (either as part of failure recovery, or an explicit rollback).
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageState {
    pub target_package: String,
    pub physical_path: PathBuf,
    pub backup_path: PathBuf,
    pub backup_hash: Option<ContentHash>,
    pub temp_path: Option<PathBuf>,
    #[serde(default)]
    pub installed_hash: Option<ContentHash>,
    pub stage: PackageStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionOutcome {
    InProgress,
    Committed,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransactionKind {
    #[default]
    Install,
    Uninstall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionState {
    #[serde(default)]
    pub kind: TransactionKind,
    pub patch_id: Uuid,
    pub patch_path: PathBuf,
    pub game_root: PathBuf,
    pub backup_dir: PathBuf,
    pub packages: Vec<PackageState>,
    pub outcome: TransactionOutcome,
    pub error: Option<String>,
}

impl TransactionState {
    pub fn new(
        patch_id: Uuid,
        patch_path: impl Into<PathBuf>,
        game_root: impl Into<PathBuf>,
        backup_dir: impl Into<PathBuf>,
        target_packages: &[(String, PathBuf)],
    ) -> Self {
        let backup_dir = backup_dir.into();
        let packages = target_packages
            .iter()
            .map(|(target_package, physical_path)| PackageState {
                target_package: target_package.clone(),
                physical_path: physical_path.clone(),
                backup_path: backup_path_for(&backup_dir, target_package),
                backup_hash: None,
                temp_path: None,
                installed_hash: None,
                stage: PackageStage::Planned,
            })
            .collect();

        Self {
            kind: TransactionKind::Install,
            patch_id,
            patch_path: patch_path.into(),
            game_root: game_root.into(),
            backup_dir,
            packages,
            outcome: TransactionOutcome::InProgress,
            error: None,
        }
    }

    pub fn new_uninstall(
        patch_id: Uuid,
        patch_path: impl Into<PathBuf>,
        game_root: impl Into<PathBuf>,
        backup_dir: impl Into<PathBuf>,
        target_packages: &[(String, PathBuf)],
    ) -> Self {
        let mut state = Self::new(patch_id, patch_path, game_root, backup_dir, target_packages);
        state.kind = TransactionKind::Uninstall;
        state
    }

    pub fn state_path(&self) -> PathBuf {
        self.backup_dir.join(TRANSACTION_STATE_FILE_NAME)
    }

    pub fn save(&self) -> Result<()> {
        let path = self.state_path();
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| PatcherError::json(&path, e))?;
        atomic_write(&path, &bytes).map_err(PatcherError::from)
    }

    pub fn load(backup_dir: impl AsRef<Path>) -> Result<Self> {
        let path = backup_dir.as_ref().join(TRANSACTION_STATE_FILE_NAME);
        let bytes = read_file(&path).map_err(PatcherError::from)?;
        serde_json::from_slice(&bytes).map_err(|e| PatcherError::json(&path, e))
    }

    pub fn try_load(backup_dir: impl AsRef<Path>) -> Option<Self> {
        let path = backup_dir.as_ref().join(TRANSACTION_STATE_FILE_NAME);
        if !path.exists() {
            return None;
        }
        Self::load(backup_dir).ok()
    }

    pub fn package_mut(&mut self, target_package: &str) -> Option<&mut PackageState> {
        self.packages
            .iter_mut()
            .find(|p| p.target_package == target_package)
    }

    pub fn packages_in_stage(&self, stage: PackageStage) -> impl Iterator<Item = &PackageState> {
        self.packages.iter().filter(move |p| p.stage == stage)
    }

    pub fn all_reached(&self, stage: PackageStage) -> bool {
        self.packages
            .iter()
            .all(|p| stage_rank(p.stage) >= stage_rank(stage))
    }
}

/// Total order over [`PackageStage`] matching its natural progression,
/// so [`TransactionState::all_reached`] can ask "has every package
/// gotten at least this far" without a giant match.
fn stage_rank(stage: PackageStage) -> u8 {
    match stage {
        PackageStage::Planned => 0,
        PackageStage::BackedUp => 1,
        PackageStage::TempBuilt => 2,
        PackageStage::SwapStarted => 3,
        PackageStage::Swapped => 4,
        PackageStage::Committed => 5,
        PackageStage::RolledBack => 5,
    }
}

/// Directory (relative to a transaction's `backup_dir`) under which
/// every touched package's backup is nested, mirroring the package's
/// own relative path/hierarchy exactly rather than flattening it into
/// a single sanitized path segment.
const BACKUP_PACKAGES_SUBDIR: &str = "packages";

///
/// A flattening scheme (e.g. replacing every `/`/`\` with `__`) is
/// *not* injective: `"a/b.cpk"` and the differently-structured but
/// textually similar `"a__b.cpk"` both flatten to `a__b.cpk`, so a
/// patch touching both would silently overwrite one package's backup
/// with the other's. `target_package` is already validated (see
/// `asset_project::manifest::TargetPackage`) to be a non-empty,
/// forward-slash-separated relative path with no absolute prefix and
/// no `.`/`..` traversal component, so joining it verbatim onto a
/// dedicated subdirectory is both collision-free (two distinct
/// `target_package` values can never produce the same backup path)
/// and at least as readable for manual inspection as the old scheme.
/// Nesting under [`BACKUP_PACKAGES_SUBDIR`] additionally guarantees no
/// package's backup can ever collide with this directory's own
/// `state.json` (or any other future bookkeeping file placed directly
/// under `backup_dir`).
fn backup_path_for(backup_dir: &Path, target_package: &str) -> PathBuf {
    let normalized = target_package.replace('\\', "/");
    backup_dir.join(BACKUP_PACKAGES_SUBDIR).join(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_round_trip() {
        let dir = crate::test_scratch::dir("transaction-state");
        let backup_dir = dir.join("backups").join("patch-1");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let mut state = TransactionState::new(
            Uuid::new_v4(),
            dir.join("update.ybpatch"),
            &dir,
            &backup_dir,
            &[("scene.cpk".to_string(), dir.join("scene.cpk"))],
        );
        state.package_mut("scene.cpk").unwrap().stage = PackageStage::BackedUp;
        state.package_mut("scene.cpk").unwrap().backup_hash = Some(ContentHash::of(b"x"));
        state.save().unwrap();

        let loaded = TransactionState::load(&backup_dir).unwrap();
        assert_eq!(loaded.packages.len(), 1);
        assert_eq!(loaded.packages[0].stage, PackageStage::BackedUp);
        assert!(loaded.packages[0].backup_hash.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn all_reached_checks_every_package() {
        let dir = crate::test_scratch::dir("transaction-state-all-reached");
        let mut state = TransactionState::new(
            Uuid::new_v4(),
            dir.join("update.ybpatch"),
            &dir,
            &dir,
            &[
                ("a.cpk".to_string(), dir.join("a.cpk")),
                ("b.cpk".to_string(), dir.join("b.cpk")),
            ],
        );
        assert!(!state.all_reached(PackageStage::Swapped));

        state.package_mut("a.cpk").unwrap().stage = PackageStage::Swapped;
        assert!(!state.all_reached(PackageStage::Swapped));

        state.package_mut("b.cpk").unwrap().stage = PackageStage::Swapped;
        assert!(state.all_reached(PackageStage::Swapped));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_path_for_mirrors_package_hierarchy() {
        let backup_dir = Path::new("/patch-backups/patch-1");
        assert_eq!(
            backup_path_for(backup_dir, "basedata/basedata.cpk"),
            backup_dir.join("packages/basedata/basedata.cpk")
        );
        // Backslash-separated input is normalized the same way.
        assert_eq!(
            backup_path_for(backup_dir, r"basedata\basedata.cpk"),
            backup_dir.join("packages/basedata/basedata.cpk")
        );
    }

    #[test]
    fn backup_path_for_does_not_collide_across_distinct_target_packages() {
        // A lossy flattening scheme (replacing every `/` with `__`)
        // would map both of these to the same path
        // (`basedata__basedata.cpk`); the hierarchy-preserving scheme
        // must keep them distinct.
        let backup_dir = Path::new("/patch-backups/patch-1");
        let nested = backup_path_for(backup_dir, "basedata/basedata.cpk");
        let flat = backup_path_for(backup_dir, "basedata__basedata.cpk");
        assert_ne!(nested, flat);
    }

    #[test]
    fn transaction_state_new_assigns_distinct_backup_paths_for_colliding_names() {
        let dir = crate::test_scratch::dir("transaction-state-backup-collision");
        let state = TransactionState::new(
            Uuid::new_v4(),
            dir.join("update.ybpatch"),
            &dir,
            &dir,
            &[
                (
                    "basedata/basedata.cpk".to_string(),
                    dir.join("basedata/basedata.cpk"),
                ),
                (
                    "basedata__basedata.cpk".to_string(),
                    dir.join("basedata__basedata.cpk"),
                ),
            ],
        );

        let find = |name: &str| {
            state
                .packages
                .iter()
                .find(|p| p.target_package == name)
                .unwrap()
                .backup_path
                .clone()
        };
        let nested = find("basedata/basedata.cpk");
        let flat = find("basedata__basedata.cpk");
        assert_ne!(
            nested, flat,
            "distinct target_package values must never share a backup path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
