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

use asset_project::journal::InstallationJournal;
use uuid::Uuid;

use crate::error::Result;
use crate::state::{TransactionOutcome, TransactionState};
use crate::transaction::{PatchPaths, restore_swapped_packages};

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
    let paths = PatchPaths::for_root(game_root);
    let mut journal = InstallationJournal::load_or_default(&paths.journal_path)?;
    let backup_dir = paths.backup_dir_for(patch_id);

    match TransactionState::try_load(&backup_dir) {
        Some(state) if state.outcome == TransactionOutcome::Committed => {
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

/// Convenience: detect and recover every pending install for
/// `game_root` in one call. Returns the list of patch ids that were
/// recovered.
pub fn recover_all_pending(game_root: &Path) -> Result<Vec<Uuid>> {
    let pending = detect_pending(game_root)?;
    let mut recovered = Vec::with_capacity(pending.len());
    for install in &pending {
        recover_interrupted(game_root, install.patch_id)?;
        recovered.push(install.patch_id);
    }
    Ok(recovered)
}
