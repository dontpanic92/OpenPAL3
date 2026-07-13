//! Cross-platform, crash-recoverable "replace one package file's
//! bytes with another's" primitive.
//!
//! `std::fs::rename` is atomic on POSIX even when the destination
//! already exists (the directory entry is swapped; any process with
//! the old file open keeps a perfectly valid handle to the old
//! inode). On Windows it is not equivalent: `std::fs::rename` does
//! pass `MOVEFILE_REPLACE_EXISTING` to `MoveFileExW`, but that call
//! (and a bare rename in general) fails outright with a sharing
//! violation if *anything* — an antivirus scanner, the search
//! indexer, Explorer's thumbnail cache, a lingering handle from the
//! game itself — has the destination package open without
//! `FILE_SHARE_DELETE`. A PAL3 install's `.cpk` files are exactly the
//! kind of file such background processes like to poke at, so this
//! module exists to make replacing them robust and, crucially,
//! recoverable if the *process itself* (not just one rename call)
//! dies partway through.
//!
//! The core idea: never rely on a single rename that targets an
//! already-existing destination. Every rename this module performs
//! targets a destination path that is guaranteed absent, which is the
//! one case `rename`/`MoveFileExW` handles identically, unambiguously,
//! and atomically on every platform. Replacing `target_path` is
//! decomposed into:
//!
//! 1. Rename `target_path` (if present) aside to a deterministic
//!    sibling name ([`pending_old_path`]) — not a random/timestamped
//!    one, so a crash here leaves a marker any later process (a
//!    fresh run of this tool, or its startup recovery pass) can find
//!    without needing to already know it exists.
//! 2. Rename the replacement file onto `target_path` (now guaranteed
//!    absent).
//! 3. Remove the aside-renamed old file. Best-effort: if this step
//!    itself is interrupted, the stray marker is harmless and gets
//!    cleaned up the next time [`replace_file`] (or
//!    [`recover_stale_replace`]) runs against the same `target_path`.
//!
//! On Windows, step 1+2 are first attempted as a single
//! [`windows::try_replace_file_native`] call (`ReplaceFileW`), the API
//! Microsoft's own docs recommend specifically for "replace this file
//! with that one" (it also preserves the destination's ACLs/
//! attributes and retries certain transient failures internally),
//! falling back to the manual sequence above if it's unavailable.

use std::fs;
use std::path::{Path, PathBuf};

use asset_project::hash::ContentHash;

use crate::error::{PatcherError, Result};
use crate::state::{PackageStage, TransactionState};

/// Deterministic (never random/timestamped) sibling path used to
/// stash `target_path`'s previous content while a replace is in
/// flight.
pub fn pending_old_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package");
    target_path.with_file_name(format!(".{file_name}.yaobowpatch.old"))
}

/// Outcome of inspecting (and, if necessary, resolving) a possibly
/// stale [`pending_old_path`] marker for `target_path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReplaceRecovery {
    /// No marker was present; nothing to do.
    NoneFound,
    /// A marker was found with `target_path` missing — a previous
    /// [`replace_file`] call was interrupted after step 1 but before
    /// step 2. Restored `target_path` from the marker, undoing the
    /// interrupted replace entirely.
    RestoredPreReplaceState,
    /// A marker was found with `target_path` already present — the
    /// previous [`replace_file`] call's step 2 had already completed
    /// (i.e. the replace itself fully succeeded) and only step 3's
    /// cleanup was interrupted. Removed the now-redundant marker;
    /// `target_path`'s content was left untouched.
    CompletedReplace,
}

/// Inspects, and resolves if necessary, a possibly-stale
/// [`pending_old_path`] marker for `target_path`. Idempotent and safe
/// to call unconditionally before any [`replace_file`] call targeting
/// the same path — [`replace_file`] does exactly this itself as its
/// first step. Also exposed directly for
/// [`reconcile_stale_replacements`] and tests.
pub fn recover_stale_replace(target_path: &Path) -> Result<StaleReplaceRecovery> {
    let old_path = pending_old_path(target_path);
    if !old_path.exists() {
        return Ok(StaleReplaceRecovery::NoneFound);
    }

    if target_path.exists() {
        fs::remove_file(&old_path).map_err(|e| PatcherError::io(&old_path, e))?;
        Ok(StaleReplaceRecovery::CompletedReplace)
    } else {
        fs::rename(&old_path, target_path).map_err(|e| PatcherError::io(target_path, e))?;
        Ok(StaleReplaceRecovery::RestoredPreReplaceState)
    }
}

/// Replaces `target_path`'s content with `new_path`'s, consuming
/// `new_path`. `new_path` must exist; `target_path` may or may not.
/// See the module doc for the full step sequence and crash-safety
/// argument.
pub fn replace_file(new_path: &Path, target_path: &Path) -> Result<()> {
    // Self-heal any marker left by a previous, interrupted call
    // against this exact `target_path` before doing anything else, so
    // step 2 below never has to contend with a leftover occupying
    // `pending_old_path`.
    recover_stale_replace(target_path)?;

    #[cfg(windows)]
    {
        match windows::try_replace_file_native(new_path, target_path) {
            Ok(()) => return Ok(()),
            Err(windows::NativeReplaceOutcome::Unavailable) => {
                // `target_path` doesn't exist yet (ReplaceFileW
                // requires it to) or the pair isn't eligible for it
                // (e.g. different volumes) — fall through to the
                // manual, always-correct sequence below.
            }
            Err(windows::NativeReplaceOutcome::Failed(e)) => {
                return Err(PatcherError::io(target_path, e));
            }
        }
    }

    replace_file_manual(new_path, target_path)
}

fn replace_file_manual(new_path: &Path, target_path: &Path) -> Result<()> {
    let old_path = pending_old_path(target_path);

    if target_path.exists() {
        fs::rename(target_path, &old_path).map_err(|e| PatcherError::io(target_path, e))?;
    }

    if let Err(e) = fs::rename(new_path, target_path) {
        // Undo step 1 so a failure here doesn't strand `target_path`
        // missing entirely.
        if old_path.exists() {
            let _ = fs::rename(&old_path, target_path);
        }
        return Err(PatcherError::io(target_path, e));
    }

    if old_path.exists() {
        if let Err(e) = fs::remove_file(&old_path) {
            log::warn!("failed to remove stale replace marker {old_path:?}: {e}");
        }
    }

    Ok(())
}

/// Scans every package in `state` for a stale [`pending_old_path`]
/// marker, *regardless of its currently recorded [`PackageStage`]*,
/// and reconciles the recorded stage with whatever
/// [`recover_stale_replace`] actually finds on disk.
///
/// This matters because a crash can happen *during* a single
/// [`replace_file`] call, not just cleanly between two whole
/// transaction steps — and stage-based recovery
/// (`transaction::restore_swapped_packages`, which only ever look at
/// packages already marked [`PackageStage::Swapped`]) would otherwise
/// never notice a package that was physically swapped (or restored)
/// on disk moments before the crash, but whose `TransactionState`
/// bookkeeping never got to record it.
///
/// The direction implied by a
/// [`StaleReplaceRecovery::CompletedReplace`] finding is inferred from
/// the package's *current* stage, which is unambiguous given this
/// crate's call graph: the only stage a forward swap
/// (`transaction::run_swap_phase`) ever calls [`replace_file`] from is
/// [`PackageStage::SwapStarted`] (persisted just before the call,
/// advancing to [`PackageStage::Swapped`] once it returns), and the
/// only stage a restore (`transaction::restore_one_package`, used by
/// both mid-apply recovery and `transaction::rollback`) ever calls it
/// from is [`PackageStage::Swapped`] (advancing it to
/// [`PackageStage::RolledBack`]) — so observing a completed replace
/// while the recorded stage is still `SwapStarted` (or, defensively,
/// `TempBuilt`) must have been an interrupted forward swap, and
/// observing one while it's `Swapped` must have been an interrupted
/// restore.
///
/// [`PackageStage::SwapStarted`] additionally needs handling in the
/// [`StaleReplaceRecovery::NoneFound`] case, which a bare "no marker,
/// nothing to do" would get wrong: `replace_file`'s own step 3 (best-
/// effort marker cleanup) can complete — meaning the swap fully
/// happened, marker included — moments before the process dies, still
/// before the caller gets to persist [`PackageStage::Swapped`]. That
/// leaves *no* marker for `recover_stale_replace` to find even though
/// the live file already holds the swapped-in bytes. This is
/// disambiguated conservatively using the package's own recorded
/// `backup_hash`: the live file's current content is re-hashed and
/// compared against it. A match means the live file still holds
/// exactly the pre-patch bytes recorded at backup time, so the rename
/// that would have changed it never ran; anything else (a mismatch,
/// or no read-able live file at all) is treated as "the swap must have
/// completed" — the safe direction to default to, since restoring an
/// already-restored package from its (independently hash-verified)
/// backup is a harmless no-op, whereas wrongly assuming an actually-
/// completed swap never happened would leave the package's patched
/// bytes in place while the journal records the whole install as
/// `Failed`.
pub(crate) fn reconcile_stale_replacements(state: &mut TransactionState) -> Result<()> {
    let target_packages: Vec<String> = state
        .packages
        .iter()
        .map(|p| p.target_package.clone())
        .collect();

    for target_package in target_packages {
        let (physical_path, backup_hash) = {
            let pkg = state
                .package_mut(&target_package)
                .expect("just collected from state.packages");
            (pkg.physical_path.clone(), pkg.backup_hash)
        };

        match recover_stale_replace(&physical_path)? {
            StaleReplaceRecovery::NoneFound => {
                let pkg = state.package_mut(&target_package).expect("present");
                if pkg.stage == PackageStage::SwapStarted {
                    let still_matches_backup = backup_hash.is_some_and(|expected| {
                        fs::read(&physical_path)
                            .map(|bytes| ContentHash::of(&bytes) == expected)
                            .unwrap_or(false)
                    });
                    let new_stage = if still_matches_backup {
                        // The live file is still byte-for-byte the
                        // pre-patch content recorded at backup time —
                        // the rename that would have changed it never
                        // ran, so nothing was actually touched.
                        PackageStage::TempBuilt
                    } else {
                        // Either the live file's content has already
                        // moved on from the backed-up original, or it
                        // couldn't be read/hashed at all — in both
                        // cases the only safe assumption is that
                        // `replace_file`'s rename (and its own marker
                        // cleanup) completed; only this struct's own
                        // `Swapped` bookkeeping write never happened.
                        PackageStage::Swapped
                    };
                    if new_stage != pkg.stage {
                        pkg.stage = new_stage;
                        state.save()?;
                    }
                }
            }
            StaleReplaceRecovery::RestoredPreReplaceState => {
                let pkg = state.package_mut(&target_package).expect("present");
                if pkg.stage == PackageStage::SwapStarted {
                    // The marker held the pre-replace bytes and
                    // `target_path` was missing, i.e. step 1 (target
                    // renamed aside) ran but step 2 (temp file renamed
                    // onto target) never did; `recover_stale_replace`
                    // already put the original bytes back, so nothing
                    // was ever actually swapped.
                    pkg.stage = PackageStage::TempBuilt;
                    state.save()?;
                }
                // `Swapped` (a restore in progress) needs no change:
                // the restore's own pre-replace state *is* the
                // swapped-in bytes, so the package legitimately
                // remains `Swapped` until the restore is retried.
            }
            StaleReplaceRecovery::CompletedReplace => {
                let pkg = state.package_mut(&target_package).expect("present");
                let previous_stage = pkg.stage;
                pkg.stage = match previous_stage {
                    PackageStage::TempBuilt | PackageStage::SwapStarted => PackageStage::Swapped,
                    PackageStage::Swapped => PackageStage::RolledBack,
                    other => {
                        log::warn!(
                            "found a completed-but-uncommitted replace for {target_package:?} \
                             while its recorded stage was {other:?}; leaving the stage as-is"
                        );
                        other
                    }
                };
                if pkg.stage != previous_stage {
                    state.save()?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use winapi::shared::winerror::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use winapi::um::errhandlingapi::{GetLastError, SetLastError};
    use winapi::um::winbase::{
        REPLACEFILE_IGNORE_ACL_ERRORS, REPLACEFILE_WRITE_THROUGH, ReplaceFileW,
    };

    pub enum NativeReplaceOutcome {
        /// `ReplaceFileW` can't be used for this pair of paths (most
        /// commonly: `target_path` doesn't exist yet, which
        /// `ReplaceFileW` requires) — caller should fall back to the
        /// manual rename sequence.
        Unavailable,
        /// `ReplaceFileW` was applicable but failed.
        Failed(io::Error),
    }

    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// `ReplaceFileW(target_path, new_path, NULL, ...)`: atomically
    /// replaces `target_path`'s content with `new_path`'s in a single
    /// call, preserving `target_path`'s ACLs/attributes and retrying
    /// certain transient sharing failures internally — the operation
    /// Microsoft's own docs recommend over a bare rename specifically
    /// for "replace this file with that one".
    ///
    /// No backup path is requested (`lpBackupFileName = NULL`): this
    /// crate already maintains its own hash-verified backup per
    /// package (`state::PackageState::backup_path`), so
    /// `ReplaceFileW`'s own transient backup would be redundant.
    ///
    /// Note: `winapi` 0.3's `ReplaceFileW` binding omits the real
    /// Win32 `BOOL` return value (declaring it `-> ()`), so success
    /// can't be read off a return value here. We clear the
    /// thread-local last-error before the call and inspect it
    /// afterwards instead — the documented way to detect success for
    /// an API whose binding doesn't expose its return value.
    pub fn try_replace_file_native(
        new_path: &Path,
        target_path: &Path,
    ) -> Result<(), NativeReplaceOutcome> {
        let target_w = to_wide(target_path);
        let new_w = to_wide(new_path);

        unsafe {
            SetLastError(ERROR_SUCCESS);
            ReplaceFileW(
                target_w.as_ptr(),
                new_w.as_ptr(),
                std::ptr::null(),
                REPLACEFILE_WRITE_THROUGH | REPLACEFILE_IGNORE_ACL_ERRORS,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }

        let err = unsafe { GetLastError() };
        if err == ERROR_SUCCESS {
            return Ok(());
        }
        if err == ERROR_FILE_NOT_FOUND {
            return Err(NativeReplaceOutcome::Unavailable);
        }
        Err(NativeReplaceOutcome::Failed(io::Error::from_raw_os_error(
            err as i32,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        crate::test_scratch::dir(name)
    }

    #[test]
    fn replaces_existing_destination_content() {
        let dir = scratch("replace-existing");
        let target = dir.join("package.cpk");
        let new = dir.join("incoming.tmp");
        std::fs::write(&target, b"old content").unwrap();
        std::fs::write(&new, b"new content").unwrap();

        replace_file(&new, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new content");
        assert!(!new.exists());
        assert!(!pending_old_path(&target).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn succeeds_when_destination_does_not_exist_yet() {
        let dir = scratch("replace-fresh");
        let target = dir.join("package.cpk");
        let new = dir.join("incoming.tmp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&new, b"brand new").unwrap();

        replace_file(&new, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"brand new");
        assert!(!new.exists());
        assert!(!pending_old_path(&target).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recover_stale_replace_is_a_no_op_when_no_marker_present() {
        let dir = scratch("replace-no-marker");
        let target = dir.join("package.cpk");
        std::fs::write(&target, b"content").unwrap();

        let outcome = recover_stale_replace(&target).unwrap();
        assert_eq!(outcome, StaleReplaceRecovery::NoneFound);
        assert_eq!(std::fs::read(&target).unwrap(), b"content");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recover_stale_replace_restores_pre_replace_state_when_target_missing() {
        // Simulates a crash between step 1 (target -> old) and step 2
        // (new -> target) of a previous `replace_file` call.
        let dir = scratch("replace-restore-missing-target");
        let target = dir.join("package.cpk");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(pending_old_path(&target), b"pre-replace original").unwrap();
        assert!(!target.exists());

        let outcome = recover_stale_replace(&target).unwrap();
        assert_eq!(outcome, StaleReplaceRecovery::RestoredPreReplaceState);
        assert_eq!(std::fs::read(&target).unwrap(), b"pre-replace original");
        assert!(!pending_old_path(&target).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recover_stale_replace_cleans_up_when_target_already_present() {
        // Simulates a crash between step 2 (new -> target, already
        // done) and step 3 (removing the now-redundant marker).
        let dir = scratch("replace-cleanup-completed");
        let target = dir.join("package.cpk");
        std::fs::write(&target, b"already-new content").unwrap();
        std::fs::write(pending_old_path(&target), b"stale original").unwrap();

        let outcome = recover_stale_replace(&target).unwrap();
        assert_eq!(outcome, StaleReplaceRecovery::CompletedReplace);
        assert_eq!(std::fs::read(&target).unwrap(), b"already-new content");
        assert!(!pending_old_path(&target).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_file_self_heals_a_stale_marker_before_replacing() {
        let dir = scratch("replace-self-heal");
        let target = dir.join("package.cpk");
        let new = dir.join("incoming.tmp");
        std::fs::create_dir_all(&dir).unwrap();
        // Stale marker from a previous interrupted call, with target
        // missing entirely.
        std::fs::write(pending_old_path(&target), b"stale pre-replace original").unwrap();
        std::fs::write(&new, b"final content").unwrap();

        replace_file(&new, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"final content");
        assert!(!new.exists());
        assert!(!pending_old_path(&target).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
