//! Startup detection of interrupted installs: scans a PAL3 root's
//! journal for `Pending` entries (an `apply()` that began but never
//! reached `Applied`/`Failed`) and, for each one, restores any
//! already-swapped packages back to their pre-patch backups so the
//! game root ends up in a consistent state regardless of when in the
//! apply sequence the previous process died.
//!
//! Intended to be called once, early, by the GUI binary on startup
//! for whatever root the user has selected (or the last-used root
//! from config) — before any new patch is opened.

use std::path::Path;

use asset_project::journal::{InstallStatus, InstallationJournal};
use asset_project::patch::YbpatchReader;
use uuid::Uuid;

use crate::error::Result;
use crate::state::{PackageStage, TransactionKind, TransactionOutcome, TransactionState};
use crate::transaction::{PatchPaths, cleanup_unswapped_temp_files, restore_swapped_packages};

#[derive(Debug, Clone)]
pub struct PendingInstall {
    pub patch_id: Uuid,
    pub patch_path: std::path::PathBuf,
    /// `None` if the previous process crashed before even the first
    /// `TransactionState` write — in that case nothing was ever
    /// touched and there is nothing to restore, only the journal entry
    /// to close out.
    pub state: Option<TransactionState>,
}

/// Lists every interrupted install still marked `Pending` in
/// `game_root`'s journal, without modifying anything.
pub fn detect_pending(game_root: &Path) -> Result<Vec<PendingInstall>> {
    let paths = PatchPaths::for_root(game_root);
    let journal = InstallationJournal::load_or_default(&paths.journal_path)?;

    Ok(journal
        .pending_entries()
        .map(|entry| {
            let backup_dir = paths.backup_dir_for(entry.patch_id);
            PendingInstall {
                patch_id: entry.patch_id,
                patch_path: entry.patch_path.clone(),
                state: TransactionState::try_load(&backup_dir),
            }
        })
        .collect())
}

/// Recovers a single interrupted install. A transaction that already
/// reached `Committed` only needs its coarse journal entry finalized;
/// otherwise any swapped packages are restored and the attempt is
/// marked failed.
pub fn recover_interrupted(game_root: &Path, patch_id: Uuid) -> Result<()> {
    let _root_lock = crate::manager::RootLock::acquire(game_root)?;
    recover_interrupted_unlocked(game_root, patch_id)
}

fn recover_interrupted_unlocked(game_root: &Path, patch_id: Uuid) -> Result<()> {
    let paths = PatchPaths::for_root(game_root);
    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let backup_dir = paths.backup_dir_for(patch_id);

    match TransactionState::try_load(&backup_dir) {
        Some(state) if state.outcome == TransactionOutcome::Committed => {
            let mut patch = YbpatchReader::open(&state.patch_path)?;
            patch.verify_all()?;
            crate::transaction::record_directory_provenance(&state, patch.manifest())?;

            let mut manager = crate::manager::ManagerState::load_or_default(game_root)?;
            manager.mark_applied(patch_id);
            for package in &state.packages {
                let hash = match package.installed_hash {
                    Some(hash) => hash,
                    None => crate::fingerprint::package_fingerprint(&package.physical_path)?,
                };
                manager.set_package_head(&package.target_package, hash);
            }
            manager.save(game_root)?;

            let applied = state
                .packages
                .iter()
                .map(|p| p.target_package.clone())
                .collect();
            journal.complete(patch_id, applied)?;
            journal.save(&paths.journal_path)?;
            return Ok(());
        }
        Some(mut state) => {
            restore_swapped_packages(&mut state)?;
            cleanup_unswapped_temp_files(&state);
            state.outcome = TransactionOutcome::Failed;
            state.error = Some(
                "installation was interrupted and has been automatically recovered".to_string(),
            );
            state.save()?;
        }
        None => {
            // Crashed before the first `TransactionState::save()` —
            // no backups were made and no package was touched, so
            // there is nothing on disk to restore.
        }
    }

    journal.fail(
        patch_id,
        "installation was interrupted and has been automatically recovered",
    )?;
    journal.save(&paths.journal_path)?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct PendingUninstall {
    pub operation_id: Uuid,
    pub patch_id: Uuid,
    pub state: TransactionState,
}

pub fn detect_pending_uninstalls(game_root: &Path) -> Result<Vec<PendingUninstall>> {
    let operations_dir = crate::manager::operations_dir(game_root);
    if !operations_dir.exists() {
        return Ok(Vec::new());
    }

    let manager = crate::manager::ManagerState::load_or_default(game_root)?;
    let journal =
        InstallationJournal::load_or_default(&PatchPaths::for_root(game_root).journal_path)?;
    let mut pending = Vec::new();
    for entry in std::fs::read_dir(&operations_dir)
        .map_err(|error| crate::error::PatcherError::io(&operations_dir, error))?
    {
        let entry =
            entry.map_err(|error| crate::error::PatcherError::io(&operations_dir, error))?;
        let Some(operation_id) = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
        else {
            continue;
        };
        let Some(state) = TransactionState::try_load(entry.path()) else {
            continue;
        };
        if state.kind != TransactionKind::Uninstall {
            continue;
        }
        let journal_applied = journal.entries().iter().any(|journal_entry| {
            journal_entry.patch_id == state.patch_id
                && journal_entry.status == InstallStatus::Applied
        });
        if state.outcome == TransactionOutcome::RolledBack
            && !manager.is_applied(state.patch_id)
            && !journal_applied
        {
            if let Err(error) = std::fs::remove_dir_all(entry.path()) {
                log::warn!(
                    "failed to remove completed uninstall operation directory {}: {error}",
                    entry.path().display()
                );
            }
            continue;
        }
        let needs_restore = state.packages.iter().any(|package| {
            matches!(
                package.stage,
                PackageStage::SwapStarted | PackageStage::Swapped
            )
        });
        if state.outcome == TransactionOutcome::InProgress
            || (state.outcome == TransactionOutcome::Failed && needs_restore)
            || (state.outcome == TransactionOutcome::RolledBack
                && (manager.is_applied(state.patch_id) || journal_applied))
        {
            pending.push(PendingUninstall {
                operation_id,
                patch_id: state.patch_id,
                state,
            });
        }
    }
    pending.sort_by_key(|operation| operation.operation_id);
    Ok(pending)
}

pub fn recover_interrupted_uninstall(game_root: &Path, operation_id: Uuid) -> Result<()> {
    let _root_lock = crate::manager::RootLock::acquire(game_root)?;
    recover_interrupted_uninstall_unlocked(game_root, operation_id)
}

fn recover_interrupted_uninstall_unlocked(game_root: &Path, operation_id: Uuid) -> Result<()> {
    let operation_dir = crate::manager::operation_dir(game_root, operation_id);
    let mut state = TransactionState::load(&operation_dir)?;
    if state.kind != TransactionKind::Uninstall {
        return Ok(());
    }

    let needs_restore = state.packages.iter().any(|package| {
        matches!(
            package.stage,
            PackageStage::SwapStarted | PackageStage::Swapped
        )
    });
    if state.outcome == TransactionOutcome::InProgress
        || (state.outcome == TransactionOutcome::Failed && needs_restore)
    {
        restore_swapped_packages(&mut state)?;
        cleanup_unswapped_temp_files(&state);
        state.outcome = TransactionOutcome::Failed;
        state.error =
            Some("uninstall was interrupted and has been automatically recovered".to_string());
        state.save()?;
        return Ok(());
    }

    if state.outcome == TransactionOutcome::RolledBack {
        cleanup_unswapped_temp_files(&state);
        let mut manager = crate::manager::ManagerState::load_or_default(game_root)?;
        manager.mark_uninstalled(state.patch_id);
        for package in &state.packages {
            let hash = match package.installed_hash {
                Some(hash) => hash,
                None => crate::fingerprint::package_fingerprint(&package.physical_path)?,
            };
            manager.set_package_head(&package.target_package, hash);
        }
        manager.save(game_root)?;

        let paths = PatchPaths::for_root(game_root);
        let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
        if journal.is_applied(state.patch_id) {
            journal.roll_back(state.patch_id)?;
            journal.save(&paths.journal_path)?;
        }
        std::fs::remove_dir_all(&operation_dir)
            .map_err(|error| crate::error::PatcherError::io(&operation_dir, error))?;
    }
    Ok(())
}

/// Convenience: detect and recover every pending install for
/// `game_root` in one call. Returns the list of patch ids that were
/// recovered.
pub fn recover_all_pending(game_root: &Path) -> Result<Vec<Uuid>> {
    let _root_lock = crate::manager::RootLock::acquire(game_root)?;
    let pending = detect_pending(game_root)?;
    let mut recovered = Vec::with_capacity(pending.len());
    for install in &pending {
        recover_interrupted_unlocked(game_root, install.patch_id)?;
        recovered.push(install.patch_id);
    }
    let pending_uninstalls = detect_pending_uninstalls(game_root)?;
    for operation in pending_uninstalls {
        recover_interrupted_uninstall_unlocked(game_root, operation.operation_id)?;
        recovered.push(operation.patch_id);
    }
    Ok(recovered)
}
