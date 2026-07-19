//! The core transactional patch-application engine.
//!
//! `apply()` sequence for a `.ybpatch` touching packages `P1..Pn`:
//!
//! 1. Open + fully hash-verify the `.ybpatch` (`YbpatchReader::open` +
//!    `verify_all`).
//! 2. Run [`crate::validate::validate`]; abort before touching any
//!    file if it reports errors.
//! 3. Record a `Pending` entry in the coarse
//!    `asset_project::journal::InstallationJournal` (persisted
//!    immediately).
//! 4. **Backup phase**: copy every `P1..Pn`'s current bytes into a
//!    patch-specific backup directory, recording each backup's hash.
//!    [`crate::state::TransactionState`] is persisted after every
//!    single package.
//! 5. **Build phase**: for every `P1..Pn`, build a sibling temp `.cpk`
//!    (via `packfs::cpk::CpkRebuilder::rebuild`, which itself verifies
//!    every edited entry against the requested bytes) and additionally
//!    reopen it here to re-check each changed entry's hash against the
//!    patch's declared `payload.content_hash`. Nothing under `P1..Pn`
//!    is modified during this phase — only sibling temp files are
//!    written. If any package fails to build, the transaction fails
//!    with **nothing swapped**, so recovery is just deleting temp
//!    files.
//! 6. **Swap phase**: only after every temp file exists and is
//!    verified, replace each live package with its temp file one at a
//!    time via [`crate::replace::replace_file`] (a crash-recoverable,
//!    cross-platform replacement primitive — see that module's docs
//!    for why a bare `fs::rename` is not sufficient on Windows),
//!    persisting state after each swap. If a swap fails partway
//!    through, every already-swapped package is restored from its
//!    backup before returning the error.
//! 7. Mark the transaction `Committed` and the journal entry
//!    `Applied`.
//!
//! [`crate::fault::FaultInjector`] lets tests force a failure at any
//! of these points to exercise step 6's recovery guarantee.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use asset_project::atomic::{atomic_write, read_file};
use asset_project::hash::ContentHash;
use asset_project::journal::InstallationJournal;
use asset_project::manifest::AssetChange;
use asset_project::patch::{PatchManifest, YbpatchReader};
use packfs::cpk::{CpkArchive, CpkEdit, CpkRebuilder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::environment::GameRoot;
use crate::error::{PatcherError, Result};
use crate::fault::{FailurePoint, FaultInjector, NoFaults};
use crate::plan::PatchPlan;
use crate::state::{PackageStage, TransactionOutcome, TransactionState};
use crate::validate::{self, ValidationSummary};

/// Directory (relative to a PAL3 root) holding this installer's
/// journal + per-patch backup directories. Namespaced under a single
/// hidden directory so it's easy for a player to spot (and exclude
/// from e.g. mod-manager scans) without scattering files across the
/// install tree.
const PATCH_STATE_DIR_NAME: &str = ".yaobow_patch";
const JOURNAL_FILE_NAME: &str = "journal.json";
const BACKUPS_DIR_NAME: &str = "backups";
const DIRECTORY_PROVENANCE_FILE_NAME: &str = "directory-provenance.json";
const DIRECTORY_PROVENANCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
struct DirectoryProvenance {
    schema_version: u32,
    packages: BTreeMap<String, BTreeSet<String>>,
}

impl DirectoryProvenance {
    fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                schema_version: DIRECTORY_PROVENANCE_SCHEMA_VERSION,
                packages: BTreeMap::new(),
            });
        }
        let bytes = read_file(path).map_err(PatcherError::from)?;
        let provenance: Self =
            serde_json::from_slice(&bytes).map_err(|error| PatcherError::json(path, error))?;
        if provenance.schema_version > DIRECTORY_PROVENANCE_SCHEMA_VERSION {
            return Err(PatcherError::Other(format!(
                "unsupported directory provenance version {} in {}",
                provenance.schema_version,
                path.display()
            )));
        }
        Ok(provenance)
    }

    fn save(&self, path: &Path) -> Result<()> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| PatcherError::json(path, error))?;
        atomic_write(path, &bytes).map_err(PatcherError::from)
    }
}

/// Well-known paths for one PAL3 root's patch bookkeeping.
#[derive(Debug, Clone)]
pub struct PatchPaths {
    pub patch_state_dir: PathBuf,
    pub journal_path: PathBuf,
    pub backups_root: PathBuf,
}

impl PatchPaths {
    pub fn for_root(game_root: &Path) -> Self {
        let patch_state_dir = game_root.join(PATCH_STATE_DIR_NAME);
        Self {
            journal_path: patch_state_dir.join(JOURNAL_FILE_NAME),
            backups_root: patch_state_dir.join(BACKUPS_DIR_NAME),
            patch_state_dir,
        }
    }

    pub fn backup_dir_for(&self, patch_id: Uuid) -> PathBuf {
        self.backups_root.join(patch_id.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationPhase {
    Validating,
    BackingUp,
    Building,
    Swapping,
    Committing,
}

#[derive(Debug, Clone)]
pub struct OperationProgress {
    pub phase: OperationPhase,
    pub completed: usize,
    pub total: usize,
    pub target_package: Option<String>,
}

pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: OperationProgress);
}

#[derive(Default)]
pub struct ApplyOptions {
    /// Test-only seam (see [`crate::fault`]). Always `None` in
    /// production.
    pub fault_injector: Option<Box<dyn FaultInjector>>,
    pub progress_reporter: Option<Arc<dyn ProgressReporter>>,
}

#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub patch_id: Uuid,
    pub touched_packages: Vec<String>,
    pub changes_applied: usize,
}

#[derive(Debug, Clone)]
pub struct RollbackReport {
    pub patch_id: Uuid,
    pub packages_restored: Vec<String>,
}

/// Validates (see [`crate::validate::validate`]) and returns the
/// summary without applying anything. The GUI's "Validate" step and
/// `apply()`'s own internal precondition both go through this.
pub fn validate_patch(
    ybpatch_path: &Path,
    game_root: &Path,
    expected_game: &str,
) -> Result<(PatchManifest, ValidationSummary)> {
    let snapshot = crate::manager::read_patch_snapshot(ybpatch_path)?;
    let manifest = snapshot.manifest.clone();
    let root = GameRoot::open(game_root);
    let manager_state = crate::manager::ManagerState::load_or_default(game_root)?;
    let applied_manifests = crate::manager::load_applied_patches(game_root, &manager_state)?
        .into_iter()
        .map(|patch| patch.manifest)
        .collect::<Vec<_>>();
    let summary = validate::validate_managed(
        &manifest,
        &root,
        expected_game,
        &manager_state,
        &applied_manifests,
    );
    Ok((manifest, summary))
}

/// Builds the dry-run [`PatchPlan`] for a `.ybpatch` without
/// validating or touching the game root.
pub fn plan_patch(ybpatch_path: &Path) -> Result<PatchPlan> {
    let mut reader = YbpatchReader::open(ybpatch_path)?;
    reader.verify_all()?;
    Ok(PatchPlan::from_manifest(reader.manifest()))
}

/// Applies `ybpatch_path` to `game_root`. See the module doc for the
/// full sequence and recovery guarantees.
pub fn apply(
    ybpatch_path: &Path,
    game_root: &Path,
    expected_game: &str,
    options: ApplyOptions,
) -> Result<ApplyReport> {
    let ApplyOptions {
        fault_injector,
        progress_reporter,
    } = options;
    let injector: Box<dyn FaultInjector> = fault_injector.unwrap_or_else(|| Box::new(NoFaults));
    report_progress(&progress_reporter, OperationPhase::Validating, 0, 1, None);
    let _root_lock = crate::manager::RootLock::acquire(game_root)?;

    let snapshot = crate::manager::read_patch_snapshot(ybpatch_path)?;
    let manifest = snapshot.manifest.clone();

    let root = GameRoot::open(game_root);
    let mut manager_state = crate::manager::ManagerState::load_or_default(game_root)?;
    let applied_manifests = crate::manager::load_applied_patches(game_root, &manager_state)?
        .into_iter()
        .map(|patch| patch.manifest)
        .collect::<Vec<_>>();
    let summary = validate::validate_managed(
        &manifest,
        &root,
        expected_game,
        &manager_state,
        &applied_manifests,
    );
    if !summary.is_ok() {
        return Err(PatcherError::ValidationFailed(describe_summary(&summary)));
    }

    let plan = PatchPlan::from_manifest(&manifest);

    // Resolve every touched package's physical path up front —
    // `validate()` already confirmed each one exists and is writable.
    // This must happen before creating the patch-state directory or
    // pending journal entry: canonical-path alias rejection is still
    // preflight validation and must leave no recovery-shaped state.
    let mut target_packages: Vec<(String, PathBuf)> = Vec::with_capacity(plan.packages.len());
    let mut resolved_identities = HashSet::new();
    for package_plan in &plan.packages {
        let name = package_plan.target_package.as_str().to_string();
        let physical = root
            .resolve_package_path(&name)
            .ok_or_else(|| PatcherError::PackageNotFound(name.clone()))?;
        if !resolved_identities.insert(package_identity(&physical)) {
            return Err(PatcherError::ValidationFailed(format!(
                "multiple target package names resolve to the same physical package: {name:?}"
            )));
        }
        target_packages.push((name, physical));
    }

    let managed_patch = crate::manager::import_snapshot(game_root, &snapshot)?;
    let mut reader = YbpatchReader::from_bytes(snapshot.bytes.clone())?;

    let paths = PatchPaths::for_root(game_root);
    fs::create_dir_all(&paths.patch_state_dir)
        .map_err(|e| PatcherError::io(&paths.patch_state_dir, e))?;

    let backup_dir = paths.backup_dir_for(manifest.patch_id);
    fs::create_dir_all(&backup_dir).map_err(|e| PatcherError::io(&backup_dir, e))?;

    let mut state = TransactionState::new(
        manifest.patch_id,
        &managed_patch.path,
        game_root,
        &backup_dir,
        &target_packages,
    );
    state.save()?;

    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let manifest_hash = ContentHash::of(
        &serde_json::to_vec(&manifest).map_err(|e| PatcherError::json(ybpatch_path, e))?,
    );
    journal.begin(
        manifest.patch_id,
        &managed_patch.path,
        manifest_hash,
        manifest.base_project_version,
    )?;
    journal.save(&paths.journal_path)?;

    if let Err(err) = run_backup_and_build_phases(
        &mut state,
        &mut reader,
        &plan,
        &*injector,
        &progress_reporter,
    ) {
        // Nothing has been swapped yet at this point by construction
        // (build phase runs to completion, or fails, strictly before
        // the swap phase starts) — recovery is just leaving the
        // (untouched) live packages alone, discarding any sibling temp
        // files a previous, already-succeeded iteration of this loop
        // built, and recording the failure.
        cleanup_unswapped_temp_files(&state);
        state.outcome = TransactionOutcome::Failed;
        state.error = Some(err.to_string());
        let _ = state.save();
        journal.fail(manifest.patch_id, err.to_string())?;
        journal.save(&paths.journal_path)?;
        return Err(err);
    }

    if let Err(err) = run_swap_phase(&mut state, &*injector, &progress_reporter) {
        // `run_swap_phase` has already restored every package it
        // itself swapped before returning; any package that never
        // reached the swap phase still has its (now useless) sibling
        // temp file sitting next to the live package, so it is
        // discarded here too.
        cleanup_unswapped_temp_files(&state);
        state.outcome = TransactionOutcome::Failed;
        state.error = Some(err.to_string());
        let _ = state.save();
        journal.fail(manifest.patch_id, err.to_string())?;
        journal.save(&paths.journal_path)?;
        return Err(err);
    }

    state.outcome = TransactionOutcome::Committed;
    state.save()?;
    record_directory_provenance(&state, &manifest)?;

    report_progress(&progress_reporter, OperationPhase::Committing, 0, 1, None);
    manager_state.mark_applied(manifest.patch_id);
    for package in &state.packages {
        let installed_hash = package.installed_hash.ok_or_else(|| {
            PatcherError::Other(format!(
                "missing installed hash for {:?} after successful swap",
                package.target_package
            ))
        })?;
        manager_state.set_package_head(&package.target_package, installed_hash);
    }
    manager_state.save(game_root)?;

    let applied_paths: Vec<String> = manifest
        .changes
        .iter()
        .map(|c| {
            format!(
                "{}/{}",
                c.target_package.as_str(),
                c.package_internal_path.as_str()
            )
        })
        .collect();
    journal.complete(manifest.patch_id, applied_paths)?;
    journal.save(&paths.journal_path)?;

    Ok(ApplyReport {
        patch_id: manifest.patch_id,
        touched_packages: target_packages.into_iter().map(|(name, _)| name).collect(),
        changes_applied: manifest.changes.len(),
    })
}

/// Backup phase (every package, in plan order) followed by the build
/// phase (every package, in plan order). Kept as one function so a
/// build failure on package 2 still leaves packages 1 and 2's backups
/// in place (harmless — they're only ever consulted via
/// `TransactionState`, never assumed present from context) while
/// guaranteeing no swap has started.
fn run_backup_and_build_phases(
    state: &mut TransactionState,
    reader: &mut YbpatchReader,
    plan: &PatchPlan,
    injector: &dyn FaultInjector,
    progress: &Option<Arc<dyn ProgressReporter>>,
) -> Result<()> {
    for (index, package_plan) in plan.packages.iter().enumerate() {
        let target_package = package_plan.target_package.as_str().to_string();
        report_progress(
            progress,
            OperationPhase::BackingUp,
            index,
            plan.packages.len(),
            Some(target_package.clone()),
        );
        backup_package(state, &target_package)?;
        if injector.should_fail(&FailurePoint::AfterBackup(
            package_plan.target_package.clone(),
        )) {
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterBackup(package_plan.target_package.clone()).describe(),
            ));
        }
    }

    for (index, package_plan) in plan.packages.iter().enumerate() {
        let target_package = package_plan.target_package.as_str().to_string();
        report_progress(
            progress,
            OperationPhase::Building,
            index,
            plan.packages.len(),
            Some(target_package.clone()),
        );
        let changes: Vec<AssetChange> = reader
            .manifest()
            .changes
            .iter()
            .filter(|c| c.target_package == package_plan.target_package)
            .cloned()
            .collect();

        let mut edits = Vec::with_capacity(changes.len());
        for change in &changes {
            let payload = reader.read_payload(change)?;
            edits.push(CpkEdit::file(
                change.package_internal_path.as_str(),
                payload,
            ));
        }

        let physical_path = state
            .packages
            .iter()
            .find(|p| p.target_package == target_package)
            .map(|p| p.physical_path.clone())
            .expect("package registered in TransactionState::new");
        let temp_path = sibling_temp_path(&physical_path);

        CpkRebuilder::rebuild(&physical_path, &temp_path, &edits).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            PatcherError::CpkRebuild {
                path: physical_path.clone(),
                source: e,
            }
        })?;

        verify_temp_package(&temp_path, &changes).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            e
        })?;

        let pkg = state
            .package_mut(&target_package)
            .expect("package registered in TransactionState::new");
        pkg.temp_path = Some(temp_path);
        pkg.stage = PackageStage::TempBuilt;
        state.save()?;

        if injector.should_fail(&FailurePoint::AfterTempBuild(
            package_plan.target_package.clone(),
        )) {
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterTempBuild(package_plan.target_package.clone()).describe(),
            ));
        }
    }

    Ok(())
}

fn run_uninstall_backup_and_build_phases(
    state: &mut TransactionState,
    install_state: &TransactionState,
    manifest: &PatchManifest,
    plan: &PatchPlan,
    injector: &dyn FaultInjector,
    progress: &Option<Arc<dyn ProgressReporter>>,
) -> Result<()> {
    for (index, package_plan) in plan.packages.iter().enumerate() {
        let target_package = package_plan.target_package.as_str().to_string();
        report_progress(
            progress,
            OperationPhase::BackingUp,
            index,
            plan.packages.len(),
            Some(target_package.clone()),
        );
        backup_package(state, &target_package)?;
        if injector.should_fail(&FailurePoint::AfterBackup(
            package_plan.target_package.clone(),
        )) {
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterBackup(package_plan.target_package.clone()).describe(),
            ));
        }
    }

    for (index, package_plan) in plan.packages.iter().enumerate() {
        let target_package = package_plan.target_package.as_str().to_string();
        report_progress(
            progress,
            OperationPhase::Building,
            index,
            plan.packages.len(),
            Some(target_package.clone()),
        );
        let installed_package = install_state
            .packages
            .iter()
            .find(|package| package.target_package.eq_ignore_ascii_case(&target_package))
            .ok_or_else(|| {
                PatcherError::Other(format!(
                    "install state has no backup for {target_package:?}"
                ))
            })?;
        verify_recorded_backup(installed_package)?;

        let changes: Vec<&AssetChange> = manifest
            .changes
            .iter()
            .filter(|change| change.target_package == package_plan.target_package)
            .collect();
        let mut edits = Vec::with_capacity(changes.len());
        for change in changes {
            if change.is_add() {
                edits.push(CpkEdit::remove_file(change.package_internal_path.as_str()));
            } else {
                let previous = read_cpk_entry(
                    &installed_package.backup_path,
                    change.package_internal_path.as_str(),
                )?;
                edits.push(CpkEdit::file(
                    change.package_internal_path.as_str(),
                    previous,
                ));
            }
        }
        let mod_created_directories =
            known_mod_created_directories(state.game_root.as_path(), &target_package)?;
        for directory in prunable_added_directories(
            &state
                .packages
                .iter()
                .find(|package| package.target_package == target_package)
                .expect("package registered in uninstall state")
                .physical_path,
            manifest
                .changes
                .iter()
                .filter(|change| change.target_package == package_plan.target_package),
            &mod_created_directories,
        )? {
            edits.push(CpkEdit::remove_directory(directory));
        }

        let physical_path = state
            .packages
            .iter()
            .find(|package| package.target_package == target_package)
            .map(|package| package.physical_path.clone())
            .expect("package registered in uninstall state");
        let temp_path = sibling_temp_path(&physical_path);
        CpkRebuilder::rebuild(&physical_path, &temp_path, &edits).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            PatcherError::CpkRebuild {
                path: physical_path.clone(),
                source: error,
            }
        })?;

        let package = state
            .package_mut(&target_package)
            .expect("package registered in uninstall state");
        package.temp_path = Some(temp_path);
        package.stage = PackageStage::TempBuilt;
        state.save()?;

        if injector.should_fail(&FailurePoint::AfterTempBuild(
            package_plan.target_package.clone(),
        )) {
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterTempBuild(package_plan.target_package.clone()).describe(),
            ));
        }
    }

    Ok(())
}

fn verify_recorded_backup(package: &crate::state::PackageState) -> Result<()> {
    let bytes = fs::read(&package.backup_path)
        .map_err(|error| PatcherError::io(&package.backup_path, error))?;
    let actual = ContentHash::of(&bytes);
    if package
        .backup_hash
        .is_some_and(|expected| expected != actual)
    {
        return Err(PatcherError::CorruptBackup(package.target_package.clone()));
    }
    Ok(())
}

fn read_cpk_entry(package_path: &Path, internal_path: &str) -> Result<Vec<u8>> {
    use std::io::Read;

    let file =
        fs::File::open(package_path).map_err(|error| PatcherError::io(package_path, error))?;
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(file)))
        .map_err(|error| PatcherError::io(package_path, error))?;
    let normalized = internal_path.replace('/', "\\");
    let mut entry = archive
        .open_str(&normalized)
        .map_err(|error| PatcherError::io(package_path.join(&normalized), error))?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| PatcherError::io(package_path.join(&normalized), error))?;
    Ok(bytes)
}

fn prunable_added_directories<'a>(
    current_package: &Path,
    changes: impl Iterator<Item = &'a AssetChange>,
    mod_created_directories: &HashSet<String>,
) -> Result<Vec<String>> {
    let current_paths = cpk_paths(current_package)?;
    let removed_files: HashSet<String> = changes
        .filter(|change| change.is_add())
        .map(|change| {
            change
                .package_internal_path
                .as_str()
                .replace('/', "\\")
                .to_lowercase()
        })
        .collect();

    let mut candidates = HashSet::new();
    for file in &removed_files {
        let components: Vec<&str> = file.split('\\').collect();
        for depth in 1..components.len() {
            let directory = components[..depth].join("\\");
            if mod_created_directories.contains(&directory) {
                candidates.insert(directory);
            }
        }
    }

    let mut removable = Vec::new();
    for directory in &candidates {
        let prefix = format!("{directory}\\");
        let has_retained_descendant = current_paths.iter().any(|path| {
            path.starts_with(&prefix) && !removed_files.contains(path) && !candidates.contains(path)
        });
        if !has_retained_descendant {
            removable.push(directory.clone());
        }
    }
    removable.sort_by_key(|path| std::cmp::Reverse(path.matches('\\').count()));
    Ok(removable)
}

fn known_mod_created_directories(
    game_root: &Path,
    target_package: &str,
) -> Result<HashSet<String>> {
    let paths = PatchPaths::for_root(game_root);
    let mut created = HashSet::new();
    for managed_patch in crate::manager::list_managed_patches(game_root)? {
        let provenance_path = paths
            .backup_dir_for(managed_patch.patch_id())
            .join(DIRECTORY_PROVENANCE_FILE_NAME);
        let provenance = DirectoryProvenance::load_or_default(&provenance_path)?;
        if let Some(directories) = provenance
            .packages
            .get(&crate::manager::normalize_package_name(target_package))
        {
            created.extend(directories.iter().cloned());
        }
    }
    Ok(created)
}

pub(crate) fn record_directory_provenance(
    state: &TransactionState,
    manifest: &PatchManifest,
) -> Result<()> {
    let path = state.backup_dir.join(DIRECTORY_PROVENANCE_FILE_NAME);
    let mut provenance = DirectoryProvenance::load_or_default(&path)?;

    for package_state in &state.packages {
        verify_recorded_backup(package_state)?;
        let backup_paths: HashSet<String> =
            cpk_paths(&package_state.backup_path)?.into_iter().collect();
        let directories = provenance
            .packages
            .entry(crate::manager::normalize_package_name(
                &package_state.target_package,
            ))
            .or_default();

        for change in manifest.changes.iter().filter(|change| {
            change.is_add()
                && change
                    .target_package
                    .as_str()
                    .eq_ignore_ascii_case(&package_state.target_package)
        }) {
            let file = change
                .package_internal_path
                .as_str()
                .replace('/', "\\")
                .to_lowercase();
            let components: Vec<&str> = file.split('\\').collect();
            for depth in 1..components.len() {
                let directory = components[..depth].join("\\");
                if !backup_paths.contains(&directory) {
                    directories.insert(directory);
                }
            }
        }
    }

    provenance.save(&path)
}

fn cpk_paths(package_path: &Path) -> Result<Vec<String>> {
    let file =
        fs::File::open(package_path).map_err(|error| PatcherError::io(package_path, error))?;
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(file)))
        .map_err(|error| PatcherError::io(package_path, error))?;
    archive
        .full_paths()
        .map(|paths| {
            paths
                .into_iter()
                .map(|path| path.replace('/', "\\").to_lowercase())
                .collect()
        })
        .map_err(|error| PatcherError::io(package_path, error))
}

fn run_swap_phase(
    state: &mut TransactionState,
    injector: &dyn FaultInjector,
    progress: &Option<Arc<dyn ProgressReporter>>,
) -> Result<()> {
    let package_names: Vec<String> = state
        .packages
        .iter()
        .map(|p| p.target_package.clone())
        .collect();

    let package_count = package_names.len();
    for (index, target_package) in package_names.into_iter().enumerate() {
        report_progress(
            progress,
            OperationPhase::Swapping,
            index,
            package_count,
            Some(target_package.clone()),
        );
        let target = asset_project::manifest::TargetPackage::new(&target_package)
            .expect("target_package was already validated when the manifest was authored");

        if injector.should_fail(&FailurePoint::BeforeSwap(target.clone())) {
            restore_swapped_packages(state)?;
            return Err(PatcherError::InjectedFault(
                FailurePoint::BeforeSwap(target).describe(),
            ));
        }

        let (physical_path, temp_path) = {
            let pkg = state.package_mut(&target_package).expect("package present");
            let temp_path = pkg
                .temp_path
                .clone()
                .expect("build phase always sets temp_path before the swap phase runs");
            (pkg.physical_path.clone(), temp_path)
        };

        // Durably record intent to swap *before* touching the
        // filesystem, closing the crash window between `replace_file`
        // completing (including its own best-effort marker cleanup)
        // and this function persisting `Swapped` below: without this,
        // a crash in that window leaves a live file that already
        // holds the swapped-in bytes but a recorded stage that never
        // advanced past `TempBuilt`, which startup recovery (which
        // only ever restores packages already marked `Swapped`) would
        // then silently miss. See `PackageStage::SwapStarted` and
        // `crate::replace::reconcile_stale_replacements`.
        {
            let pkg = state.package_mut(&target_package).expect("package present");
            pkg.stage = PackageStage::SwapStarted;
        }
        state.save()?;

        if let Err(e) = crate::replace::replace_file(&temp_path, &physical_path) {
            restore_swapped_packages(state)?;
            return Err(e);
        }

        let bookkeeping = (|| {
            let pkg = state.package_mut(&target_package).expect("package present");
            pkg.stage = PackageStage::Swapped;
            pkg.installed_hash = Some(crate::fingerprint::package_fingerprint(&physical_path)?);
            state.save()
        })();
        if let Err(error) = bookkeeping {
            if let Err(recovery_error) = restore_swapped_packages(state) {
                return Err(PatcherError::Other(format!(
                    "{error}; additionally failed to restore swapped packages: {recovery_error}"
                )));
            }
            return Err(error);
        }

        if injector.should_fail(&FailurePoint::AfterSwap(target.clone())) {
            restore_swapped_packages(state)?;
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterSwap(target).describe(),
            ));
        }
    }

    Ok(())
}

/// Deletes every package's sibling temp `.cpk` that was built but
/// never consumed by a successful swap (i.e. every package still
/// short of [`PackageStage::Swapped`] that nonetheless has a
/// `temp_path` recorded), so a failed `apply()` never leaves stray
/// `*.yaobowpatch.tmp` files scattered next to the live packages.
/// Best-effort: a delete failure here is logged and otherwise ignored,
/// since the original error being reported to the caller always takes
/// priority and a leftover temp file is cosmetic, not a correctness
/// issue (it is never referenced by anything once the transaction is
/// marked `Failed`).
pub(crate) fn cleanup_unswapped_temp_files(state: &TransactionState) {
    for pkg in &state.packages {
        if pkg.stage == PackageStage::Swapped || pkg.stage == PackageStage::Committed {
            continue;
        }
        if let Some(temp_path) = &pkg.temp_path {
            if temp_path.exists() {
                if let Err(e) = fs::remove_file(temp_path) {
                    log::warn!("failed to remove leftover temp file {temp_path:?}: {e}");
                }
            }
        }
    }
}

/// Restores every package currently in [`PackageStage::Swapped`] back
/// to its pre-patch backup, in place, marking each
/// [`PackageStage::RolledBack`] as it goes. Used both by mid-`apply()`
/// failure recovery and by [`crate::startup::recover_interrupted`].
/// Idempotent: packages not in `Swapped` (never touched, or already
/// restored) are left alone.
///
/// Reconciles any [`crate::replace`] marker left by a crash *during*
/// [`crate::replace::replace_file`] itself first
/// ([`crate::replace::reconcile_stale_replacements`]), so a package
/// whose swap physically completed on disk moments before a crash —
/// but whose stage never advanced past `TempBuilt` to record it — is
/// still correctly picked up here rather than silently left
/// half-installed.
pub(crate) fn restore_swapped_packages(state: &mut TransactionState) -> Result<()> {
    crate::replace::reconcile_stale_replacements(state)?;

    let swapped: Vec<String> = state
        .packages_in_stage(PackageStage::Swapped)
        .map(|p| p.target_package.clone())
        .collect();

    for target_package in swapped {
        restore_one_package(state, &target_package)?;
    }

    Ok(())
}

/// Restores a single package from its backup, verifying the backup's
/// integrity first (refusing to restore from a corrupt backup) and the
/// restored file's integrity after (the temp-write-then-rename dance
/// mirrors `asset_project::atomic::atomic_write`, just for a package
/// file rather than a JSON document).
fn restore_one_package(state: &mut TransactionState, target_package: &str) -> Result<()> {
    let (physical_path, backup_path, expected_hash) = {
        let pkg = state
            .package_mut(target_package)
            .ok_or_else(|| PatcherError::Other(format!("unknown package {target_package:?}")))?;
        (
            pkg.physical_path.clone(),
            pkg.backup_path.clone(),
            pkg.backup_hash,
        )
    };

    let backup_bytes = fs::read(&backup_path).map_err(|e| PatcherError::io(&backup_path, e))?;
    let actual_backup_hash = ContentHash::of(&backup_bytes);
    if let Some(expected) = expected_hash {
        if expected != actual_backup_hash {
            return Err(PatcherError::CorruptBackup(target_package.to_string()));
        }
    }

    let temp_path = sibling_temp_path(&physical_path);
    fs::write(&temp_path, &backup_bytes).map_err(|e| PatcherError::io(&temp_path, e))?;

    let written_back = fs::read(&temp_path).map_err(|e| PatcherError::io(&temp_path, e))?;
    if ContentHash::of(&written_back) != actual_backup_hash {
        let _ = fs::remove_file(&temp_path);
        return Err(PatcherError::CorruptBackup(target_package.to_string()));
    }

    crate::replace::replace_file(&temp_path, &physical_path)?;

    let pkg = state
        .package_mut(target_package)
        .expect("checked present above");
    pkg.stage = PackageStage::RolledBack;
    state.save()?;

    Ok(())
}

/// Copies `target_package`'s current on-disk bytes into the
/// transaction's backup directory and records the backup's hash.
fn backup_package(state: &mut TransactionState, target_package: &str) -> Result<()> {
    let (physical_path, backup_path) = {
        let pkg = state
            .package_mut(target_package)
            .expect("package registered in TransactionState::new");
        (pkg.physical_path.clone(), pkg.backup_path.clone())
    };

    let bytes = fs::read(&physical_path).map_err(|e| PatcherError::io(&physical_path, e))?;
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|e| PatcherError::io(parent, e))?;
    }
    fs::write(&backup_path, &bytes).map_err(|e| PatcherError::io(&backup_path, e))?;

    // Read the backup back rather than trusting the just-written
    // buffer, so a failure partway through the write is caught here
    // instead of silently trusted.
    let written = fs::read(&backup_path).map_err(|e| PatcherError::io(&backup_path, e))?;
    let hash = ContentHash::of(&written);
    if hash != ContentHash::of(&bytes) {
        return Err(PatcherError::Other(format!(
            "backup of {target_package:?} did not round-trip byte-for-byte"
        )));
    }

    let pkg = state
        .package_mut(target_package)
        .expect("checked present above");
    pkg.backup_hash = Some(hash);
    pkg.stage = PackageStage::BackedUp;
    state.save()?;

    Ok(())
}

/// Reopens `temp_path` (a freshly-built sibling temp `.cpk`) and
/// checks every one of `changes`' declared `payload.content_hash`
/// against what's actually stored at its `package_internal_path`.
/// `CpkRebuilder::rebuild` already performs an internal byte-for-byte
/// verification against the exact bytes it was asked to write, so this
/// is a second, independent confirmation that those bytes are in turn
/// the ones the `.ybpatch` actually declared (closing the loop:
/// `.ybpatch` payload hash -> bytes handed to the rebuilder -> bytes
/// landed in the temp package).
fn verify_temp_package(temp_path: &Path, changes: &[AssetChange]) -> Result<()> {
    let file = fs::File::open(temp_path).map_err(|e| PatcherError::io(temp_path, e))?;
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(file)))
        .map_err(|e| PatcherError::io(temp_path, e))?;

    for change in changes {
        use std::io::Read;
        let internal = change.package_internal_path.as_str().replace('/', "\\");
        let mut entry = archive
            .open_str(&internal)
            .map_err(|e| PatcherError::io(temp_path, e))?;
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| PatcherError::io(temp_path, e))?;

        let actual = ContentHash::of(&buf);
        if actual != change.payload.content_hash {
            return Err(PatcherError::Other(format!(
                "post-build verification failed for {:?}: expected hash {}, found {}",
                change.package_internal_path.as_str(),
                change.payload.content_hash.to_hex(),
                actual.to_hex()
            )));
        }
    }

    Ok(())
}

/// Compatibility alias for [`uninstall`] using production options.
pub fn rollback(game_root: &Path, patch_id: Uuid) -> Result<RollbackReport> {
    uninstall(game_root, patch_id, ApplyOptions::default())
}

/// Transactionally uninstalls one managed patch by reversing only the
/// files that patch changed, preserving disjoint files installed by
/// other patches in the same CPK.
pub fn uninstall(
    game_root: &Path,
    patch_id: Uuid,
    options: ApplyOptions,
) -> Result<RollbackReport> {
    let ApplyOptions {
        fault_injector,
        progress_reporter,
    } = options;
    let injector: Box<dyn FaultInjector> = fault_injector.unwrap_or_else(|| Box::new(NoFaults));
    report_progress(&progress_reporter, OperationPhase::Validating, 0, 1, None);
    let _root_lock = crate::manager::RootLock::acquire(game_root)?;
    let paths = PatchPaths::for_root(game_root);
    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let mut manager_state = crate::manager::ManagerState::load_or_default(game_root)?;
    if !manager_state.is_applied(patch_id) || !journal.is_applied(patch_id) {
        return Err(PatcherError::PatchNotApplied(patch_id));
    }

    let managed_patch = crate::manager::load_managed_patch(game_root, patch_id)?;
    let target_keys: HashSet<(String, String)> = managed_patch
        .manifest
        .changes
        .iter()
        .map(|change| {
            (
                crate::manager::normalize_package_name(change.target_package.as_str()),
                crate::manager::normalize_internal_path(change.package_internal_path.as_str()),
            )
        })
        .collect();
    for other in crate::manager::load_applied_patches(game_root, &manager_state)? {
        if other.patch_id() == patch_id {
            continue;
        }
        for change in &other.manifest.changes {
            let key = (
                crate::manager::normalize_package_name(change.target_package.as_str()),
                crate::manager::normalize_internal_path(change.package_internal_path.as_str()),
            );
            if target_keys.contains(&key) {
                return Err(PatcherError::ValidationFailed(format!(
                    "cannot uninstall patch {patch_id}: patch {} also changes {}/{}",
                    other.patch_id(),
                    change.target_package.as_str(),
                    change.package_internal_path.as_str()
                )));
            }
        }
    }

    let install_state = TransactionState::try_load(paths.backup_dir_for(patch_id))
        .ok_or(PatcherError::NoBackupsForPatch(patch_id))?;
    let root = GameRoot::open(game_root);
    let plan = PatchPlan::from_manifest(&managed_patch.manifest);
    let mut target_packages = Vec::with_capacity(plan.packages.len());
    for package_plan in &plan.packages {
        let target_package = package_plan.target_package.as_str().to_string();
        let physical_path = root
            .resolve_package_path(&target_package)
            .ok_or_else(|| PatcherError::PackageNotFound(target_package.clone()))?;
        let actual = crate::fingerprint::package_fingerprint(&physical_path)?;
        let expected = manager_state.package_head(&target_package).ok_or_else(|| {
            PatcherError::ValidationFailed(format!(
                "no managed package head is recorded for {target_package:?}"
            ))
        })?;
        if actual != expected {
            return Err(PatcherError::FingerprintMismatch {
                target_package,
                expected: expected.to_hex(),
                actual: actual.to_hex(),
            });
        }
        target_packages.push((
            package_plan.target_package.as_str().to_string(),
            physical_path,
        ));
    }

    let operation_id = Uuid::new_v4();
    let operation_dir = crate::manager::operation_dir(game_root, operation_id);
    fs::create_dir_all(&operation_dir).map_err(|error| PatcherError::io(&operation_dir, error))?;
    let mut state = TransactionState::new_uninstall(
        patch_id,
        &managed_patch.path,
        game_root,
        &operation_dir,
        &target_packages,
    );
    state.save()?;

    if let Err(error) = run_uninstall_backup_and_build_phases(
        &mut state,
        &install_state,
        &managed_patch.manifest,
        &plan,
        &*injector,
        &progress_reporter,
    ) {
        cleanup_unswapped_temp_files(&state);
        state.outcome = TransactionOutcome::Failed;
        state.error = Some(error.to_string());
        let _ = state.save();
        remove_failed_operation_dir(&operation_dir);
        return Err(error);
    }

    if let Err(error) = run_swap_phase(&mut state, &*injector, &progress_reporter) {
        cleanup_unswapped_temp_files(&state);
        state.outcome = TransactionOutcome::Failed;
        state.error = Some(error.to_string());
        let _ = state.save();
        if !state.packages.iter().any(|package| {
            matches!(
                package.stage,
                PackageStage::SwapStarted | PackageStage::Swapped
            )
        }) {
            remove_failed_operation_dir(&operation_dir);
        }
        return Err(error);
    }

    state.outcome = TransactionOutcome::RolledBack;
    state.save()?;

    report_progress(&progress_reporter, OperationPhase::Committing, 0, 1, None);
    manager_state.mark_uninstalled(patch_id);
    for package in &state.packages {
        let hash = package.installed_hash.ok_or_else(|| {
            PatcherError::Other(format!(
                "missing post-uninstall hash for {:?}",
                package.target_package
            ))
        })?;
        manager_state.set_package_head(&package.target_package, hash);
    }

    manager_state.save(game_root)?;

    journal.roll_back(patch_id)?;
    journal.save(&paths.journal_path)?;

    let report = RollbackReport {
        patch_id,
        packages_restored: state
            .packages
            .iter()
            .map(|p| p.target_package.clone())
            .collect(),
    };
    if let Err(error) = fs::remove_dir_all(&operation_dir) {
        log::warn!(
            "failed to remove completed uninstall operation directory {}: {error}",
            operation_dir.display()
        );
    }
    Ok(report)
}

fn remove_failed_operation_dir(operation_dir: &Path) {
    if let Err(error) = fs::remove_dir_all(operation_dir) {
        log::warn!(
            "failed to remove inactive uninstall operation directory {}: {error}",
            operation_dir.display()
        );
    }
}

fn report_progress(
    reporter: &Option<Arc<dyn ProgressReporter>>,
    phase: OperationPhase,
    completed: usize,
    total: usize,
    target_package: Option<String>,
) {
    if let Some(reporter) = reporter {
        reporter.report(OperationProgress {
            phase,
            completed,
            total,
            target_package,
        });
    }
}

fn package_identity(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

/// Lists every journal entry recorded for `game_root`, most recent
/// first — used by the GUI to offer rollback of previously applied
/// patches.
pub fn list_journal_entries(game_root: &Path) -> Result<Vec<asset_project::journal::JournalEntry>> {
    let paths = PatchPaths::for_root(game_root);
    let journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let mut entries: Vec<_> = journal.entries().to_vec();
    entries.reverse();
    Ok(entries)
}

fn sibling_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package");
    let unique = format!(
        ".{}.{}.{}.yaobowpatch.tmp",
        file_name,
        std::process::id(),
        unix_now_nanos()
    );
    path.with_file_name(unique)
}

fn unix_now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn describe_summary(summary: &ValidationSummary) -> String {
    let messages: Vec<String> = summary
        .all_issues()
        .filter(|i| i.severity == validate::Severity::Error)
        .map(|i| i.message.clone())
        .collect();
    if messages.is_empty() {
        "validation failed".to_string()
    } else {
        messages.join("; ")
    }
}
