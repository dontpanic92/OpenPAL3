//! The core transactional patch-application engine.
//!
//! `apply()` sequence for a `.yapatch` touching packages `P1..Pn`:
//!
//! 1. Open + fully hash-verify the `.yapatch` (`YapatchReader::open` +
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

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use asset_project::hash::ContentHash;
use asset_project::journal::{InstallStatus, InstallationJournal};
use asset_project::manifest::AssetChange;
use asset_project::patch::{PatchManifest, YapatchReader};
use packfs::cpk::{CpkArchive, CpkEdit, CpkRebuilder};
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

#[derive(Default)]
pub struct ApplyOptions {
    /// Test-only seam (see [`crate::fault`]). Always `None` in
    /// production.
    pub fault_injector: Option<Box<dyn FaultInjector>>,
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
    yapatch_path: &Path,
    game_root: &Path,
    expected_game: &str,
) -> Result<(PatchManifest, ValidationSummary)> {
    let mut reader = YapatchReader::open(yapatch_path)?;
    reader.verify_all()?;
    let manifest = reader.manifest().clone();
    let root = GameRoot::open(game_root);
    let summary = validate::validate(&manifest, &root, expected_game);
    Ok((manifest, summary))
}

/// Builds the dry-run [`PatchPlan`] for a `.yapatch` without
/// validating or touching the game root.
pub fn plan_patch(yapatch_path: &Path) -> Result<PatchPlan> {
    let mut reader = YapatchReader::open(yapatch_path)?;
    reader.verify_all()?;
    Ok(PatchPlan::from_manifest(reader.manifest()))
}

/// Applies `yapatch_path` to `game_root`. See the module doc for the
/// full sequence and recovery guarantees.
pub fn apply(
    yapatch_path: &Path,
    game_root: &Path,
    expected_game: &str,
    options: ApplyOptions,
) -> Result<ApplyReport> {
    let injector: Box<dyn FaultInjector> =
        options.fault_injector.unwrap_or_else(|| Box::new(NoFaults));

    let mut reader = YapatchReader::open(yapatch_path)?;
    reader.verify_all()?;
    let manifest = reader.manifest().clone();

    let root = GameRoot::open(game_root);
    let summary = validate::validate(&manifest, &root, expected_game);
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

    let paths = PatchPaths::for_root(game_root);
    fs::create_dir_all(&paths.patch_state_dir)
        .map_err(|e| PatcherError::io(&paths.patch_state_dir, e))?;

    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let manifest_hash = ContentHash::of(
        &serde_json::to_vec(&manifest).map_err(|e| PatcherError::json(yapatch_path, e))?,
    );
    journal.begin(
        manifest.patch_id,
        yapatch_path,
        manifest_hash,
        manifest.base_project_version,
    )?;
    journal.save(&paths.journal_path)?;

    let backup_dir = paths.backup_dir_for(manifest.patch_id);
    fs::create_dir_all(&backup_dir).map_err(|e| PatcherError::io(&backup_dir, e))?;

    let mut state = TransactionState::new(
        manifest.patch_id,
        yapatch_path,
        game_root,
        &backup_dir,
        &target_packages,
    );
    state.save()?;

    if let Err(err) = run_backup_and_build_phases(&mut state, &mut reader, &plan, &*injector) {
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

    if let Err(err) = run_swap_phase(&mut state, &*injector) {
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
    reader: &mut YapatchReader,
    plan: &PatchPlan,
    injector: &dyn FaultInjector,
) -> Result<()> {
    for package_plan in &plan.packages {
        let target_package = package_plan.target_package.as_str().to_string();
        backup_package(state, &target_package)?;
        if injector.should_fail(&FailurePoint::AfterBackup(
            package_plan.target_package.clone(),
        )) {
            return Err(PatcherError::InjectedFault(
                FailurePoint::AfterBackup(package_plan.target_package.clone()).describe(),
            ));
        }
    }

    for package_plan in &plan.packages {
        let target_package = package_plan.target_package.as_str().to_string();
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

fn run_swap_phase(state: &mut TransactionState, injector: &dyn FaultInjector) -> Result<()> {
    let package_names: Vec<String> = state
        .packages
        .iter()
        .map(|p| p.target_package.clone())
        .collect();

    for target_package in package_names {
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

        {
            let pkg = state.package_mut(&target_package).expect("package present");
            pkg.stage = PackageStage::Swapped;
        }
        state.save()?;

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
fn cleanup_unswapped_temp_files(state: &TransactionState) {
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
/// the ones the `.yapatch` actually declared (closing the loop:
/// `.yapatch` payload hash -> bytes handed to the rebuilder -> bytes
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

/// Rolls back a previously `Applied` patch: restores every touched
/// package from its patch-specific backup (verifying each backup's
/// hash first), then marks the journal entry `RolledBack`. Safe to
/// call again if interrupted partway — already-restored packages are
/// skipped.
///
/// Rejected outright (before touching anything) if a *newer* patch
/// (i.e. one recorded later in the journal, per journal ordering)
/// that is still `Applied` touches any package this patch also
/// touches: rolling `patch_id` back underneath it would silently
/// discard the newer patch's changes to that package while leaving
/// its journal entry claiming `Applied`. Roll back the newer,
/// overlapping patch(es) first — that is always safe to do, and once
/// done this patch can be rolled back on its own.
pub fn rollback(game_root: &Path, patch_id: Uuid) -> Result<RollbackReport> {
    let paths = PatchPaths::for_root(game_root);
    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    if !journal.is_applied(patch_id) {
        return Err(PatcherError::PatchNotApplied(patch_id));
    }

    let conflicts = overlapping_newer_applied_patches(&journal, &paths, patch_id)?;
    if !conflicts.is_empty() {
        let mut blocking_patch_ids: Vec<Uuid> = conflicts.iter().map(|(id, _)| *id).collect();
        blocking_patch_ids.sort();
        let mut overlapping_packages: Vec<String> = conflicts
            .into_iter()
            .flat_map(|(_, packages)| packages)
            .collect();
        overlapping_packages.sort();
        overlapping_packages.dedup();
        return Err(PatcherError::RollbackBlockedByNewerPatch {
            patch_id,
            blocking_patch_ids,
            overlapping_packages,
        });
    }

    let backup_dir = paths.backup_dir_for(patch_id);
    let mut state =
        TransactionState::try_load(&backup_dir).ok_or(PatcherError::NoBackupsForPatch(patch_id))?;

    // Reconcile any package whose restore was interrupted mid-`replace_file`
    // on a previous (crashed) rollback attempt before deciding what
    // still needs restoring — see
    // `crate::replace::reconcile_stale_replacements`.
    crate::replace::reconcile_stale_replacements(&mut state)?;

    let to_restore: Vec<String> = state
        .packages
        .iter()
        .filter(|p| p.stage != PackageStage::RolledBack)
        .map(|p| p.target_package.clone())
        .collect();

    for target_package in &to_restore {
        restore_one_package(&mut state, target_package)?;
    }

    state.outcome = TransactionOutcome::RolledBack;
    state.save()?;

    journal.roll_back(patch_id)?;
    journal.save(&paths.journal_path)?;

    Ok(RollbackReport {
        patch_id,
        packages_restored: state
            .packages
            .iter()
            .map(|p| p.target_package.clone())
            .collect(),
    })
}

/// Every currently-`Applied` patch recorded strictly *after*
/// `patch_id`'s own (most recent) journal entry that touches at least
/// one package `patch_id` also touches, paired with the overlapping
/// package name(s) — i.e. every reason rolling `patch_id` back right
/// now would be unsafe. Journal order (not `started_at`/wall-clock
/// time, which can be identical or even skewed across retries) is the
/// source of truth for "newer": entries are only ever appended, so a
/// later index always means a later (or retried-later) attempt.
///
/// Each candidate patch's touched-package set is read from its own
/// [`TransactionState`] (kept forever under its own backup directory,
/// even after that patch is itself rolled back) rather than
/// `JournalEntry::changes_applied` — that field concatenates
/// `target_package` and `package_internal_path` with a bare `/`,
/// which is ambiguous to split back apart, whereas `TransactionState`
/// already records each package's `target_package` as its own field.
fn overlapping_newer_applied_patches(
    journal: &InstallationJournal,
    paths: &PatchPaths,
    patch_id: Uuid,
) -> Result<Vec<(Uuid, Vec<String>)>> {
    let entries = journal.entries();
    let Some(idx) = entries.iter().rposition(|e| e.patch_id == patch_id) else {
        return Ok(Vec::new());
    };

    let target_state = TransactionState::load(paths.backup_dir_for(patch_id))?;
    let target_packages: HashSet<String> = target_state
        .packages
        .iter()
        .map(|p| package_identity(&p.physical_path))
        .collect();
    if target_packages.is_empty() {
        return Ok(Vec::new());
    }

    let mut conflicts = Vec::new();
    for entry in &entries[idx + 1..] {
        if entry.status != InstallStatus::Applied || entry.patch_id == patch_id {
            continue;
        }
        let other_state = TransactionState::load(paths.backup_dir_for(entry.patch_id))?;
        let overlap: Vec<String> = other_state
            .packages
            .iter()
            .filter(|package| target_packages.contains(&package_identity(&package.physical_path)))
            .map(|package| package.target_package.clone())
            .collect();
        if !overlap.is_empty() {
            conflicts.push((entry.patch_id, overlap));
        }
    }

    Ok(conflicts)
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
