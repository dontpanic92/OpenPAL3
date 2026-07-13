//! Startup recovery of installs interrupted *mid-`replace_file`* —
//! i.e. a crash inside a single file-replacement call, not just
//! cleanly between two whole transaction steps. This is the scenario
//! `crate::replace::reconcile_stale_replacements` exists for: a stray
//! `crate::replace::pending_old_path` marker left on disk, with the
//! package's recorded `PackageStage` not (yet) reflecting what
//! actually happened physically.
//!
//! `startup_recovery.rs` already covers the coarser "crashed cleanly
//! between swap of package 1 and swap of package 2" case. These tests
//! instead reconstruct the two ways a crash can land *inside* a single
//! `replace_file` call, per its module doc:
//!
//! * forward swap (`run_swap_phase`, stage still `TempBuilt`) crashing
//!   after its replace's step 2 (new content already in place) but
//!   before step 3 (marker cleanup) -- a "completed but unrecorded"
//!   swap.
//! * restore (`restore_one_package`, used by both recovery and
//!   `rollback`, stage still `Swapped`) crashing after its replace's
//!   step 1 (old/swapped content moved aside to the marker) but before
//!   step 2 (backup content moved into place) -- the live file is
//!   momentarily missing entirely.

mod support;

use asset_project::journal::InstallationJournal;
use yaobow_asset_patcher::replace::pending_old_path;
use yaobow_asset_patcher::startup;
use yaobow_asset_patcher::state::{PackageStage, TransactionOutcome, TransactionState};
use yaobow_asset_patcher::transaction::PatchPaths;

/// Shared setup: a one-package `.yapatch`, a `Pending` journal entry,
/// and a `TransactionState` (with the package's backup already
/// written) ready for a test to push into whatever mid-replace state
/// it wants to exercise.
struct Fixture {
    env: support::TestEnv,
    p1_path: std::path::PathBuf,
    p1_original: Vec<u8>,
    patch_id: uuid::Uuid,
    paths: PatchPaths,
    backup_dir: std::path::PathBuf,
}

fn build_fixture(name: &str) -> Fixture {
    let env = support::TestEnv::new(name);
    let (p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"p1 original" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[("p1.cpk", p1_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "p1.cpk", "new.dat", b"p1 new",
        )],
    );
    let manifest = asset_project::patch::YapatchReader::open(&patch_path)
        .unwrap()
        .manifest()
        .clone();
    let patch_id = manifest.patch_id;

    let paths = PatchPaths::for_root(&env.game_root);
    std::fs::create_dir_all(&paths.patch_state_dir).unwrap();
    let backup_dir = paths.backup_dir_for(patch_id);
    std::fs::create_dir_all(&backup_dir).unwrap();

    let p1_original = std::fs::read(&p1_path).unwrap();

    let mut state = TransactionState::new(
        patch_id,
        &patch_path,
        &env.game_root,
        &backup_dir,
        &[("p1.cpk".to_string(), p1_path.clone())],
    );
    {
        let pkg = state.package_mut("p1.cpk").unwrap();
        std::fs::create_dir_all(pkg.backup_path.parent().unwrap()).unwrap();
        std::fs::write(&pkg.backup_path, &p1_original).unwrap();
        pkg.backup_hash = Some(support::whole_file_hash(&pkg.backup_path));
    }
    state.save().unwrap();

    let mut journal = InstallationJournal::new();
    let manifest_hash =
        asset_project::hash::ContentHash::of(&serde_json::to_vec(&manifest).unwrap());
    journal
        .begin(
            patch_id,
            &patch_path,
            manifest_hash,
            manifest.base_project_version,
        )
        .unwrap();
    journal.save(&paths.journal_path).unwrap();

    Fixture {
        env,
        p1_path,
        p1_original,
        patch_id,
        paths,
        backup_dir,
    }
}

#[test]
fn recovers_a_forward_swap_interrupted_between_replace_steps_2_and_3() {
    let fx = build_fixture("replace-recover-forward-swap-interrupted");

    // Reproduce the exact on-disk shape of a crash inside
    // `replace_file` right after step 2 (new content already renamed
    // onto the live package) but before step 3 (marker cleanup):
    // stage is still `TempBuilt` (the caller only advances it to
    // `Swapped` *after* `replace_file` returns), the live file already
    // holds the swapped-in bytes, and a stale marker holding the
    // pre-replace bytes is still sitting next to it.
    let mut state = TransactionState::load(&fx.backup_dir).unwrap();
    state.package_mut("p1.cpk").unwrap().stage = PackageStage::TempBuilt;
    state.save().unwrap();

    std::fs::write(pending_old_path(&fx.p1_path), &fx.p1_original).unwrap();
    std::fs::write(&fx.p1_path, b"p1 swapped (simulated post-crash state)").unwrap();

    // Sanity: nothing has run recovery yet.
    assert!(pending_old_path(&fx.p1_path).exists());

    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();

    // The interrupted-but-physically-completed swap must be both
    // recognized (stage reconciled from `TempBuilt` to `Swapped`) and
    // then rolled back like any other completed-but-uncommitted swap,
    // landing the live file back on its original pre-patch bytes.
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);
    assert!(
        !pending_old_path(&fx.p1_path).exists(),
        "stale replace marker must not survive recovery"
    );

    let recovered_state = TransactionState::load(&fx.backup_dir).unwrap();
    assert_eq!(recovered_state.outcome, TransactionOutcome::Failed);
    let p1_stage = recovered_state
        .packages
        .iter()
        .find(|p| p.target_package == "p1.cpk")
        .map(|p| p.stage);
    assert_eq!(p1_stage, Some(PackageStage::RolledBack));

    let journal_after = InstallationJournal::load_or_default(&fx.paths.journal_path).unwrap();
    assert_eq!(
        journal_after.entries()[0].status,
        asset_project::journal::InstallStatus::Failed
    );

    // Idempotent: recovering an already-recovered install must not
    // error or disturb the now-correct state.
    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);
}

#[test]
fn recovers_a_forward_swap_that_completed_and_cleaned_up_its_marker_before_the_crash() {
    // This reproduces the exact crash window `PackageStage::SwapStarted`
    // exists to close: `replace_file` can complete *all three* of its
    // own steps -- including its own best-effort marker cleanup --
    // and the process can die right after, before `run_swap_phase`
    // gets to persist `PackageStage::Swapped`. Unlike the
    // steps-2-and-3 scenario above, there is no stale marker left
    // behind at all here: `recover_stale_replace` reports `NoneFound`.
    let fx = build_fixture("replace-recover-swap-started-marker-already-gone");

    let mut state = TransactionState::load(&fx.backup_dir).unwrap();
    state.package_mut("p1.cpk").unwrap().stage = PackageStage::SwapStarted;
    state.save().unwrap();

    // No marker: `replace_file`'s step 3 already ran to completion.
    assert!(!pending_old_path(&fx.p1_path).exists());
    std::fs::write(&fx.p1_path, b"p1 swapped (simulated post-crash state)").unwrap();

    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();

    // Before this fix, `reconcile_stale_replacements` treated
    // `NoneFound` as always meaning "nothing to do", so a package
    // stuck in this exact window would never be recognized as
    // swapped and would be left with its post-patch bytes in place
    // while the whole install was recorded `Failed` -- a silent,
    // permanent inconsistency. It must instead be detected (via the
    // live file's content no longer matching the recorded
    // `backup_hash`) and rolled back like any other completed-but-
    // uncommitted swap.
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);

    let recovered_state = TransactionState::load(&fx.backup_dir).unwrap();
    assert_eq!(recovered_state.outcome, TransactionOutcome::Failed);
    let p1_stage = recovered_state
        .packages
        .iter()
        .find(|p| p.target_package == "p1.cpk")
        .map(|p| p.stage);
    assert_eq!(p1_stage, Some(PackageStage::RolledBack));

    let journal_after = InstallationJournal::load_or_default(&fx.paths.journal_path).unwrap();
    assert_eq!(
        journal_after.entries()[0].status,
        asset_project::journal::InstallStatus::Failed
    );

    // Idempotent, same as the other crash-window scenarios.
    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);
}

#[test]
fn recovers_a_forward_swap_that_never_actually_started() {
    // The other half of the same crash window: `PackageStage::SwapStarted`
    // was durably persisted, but the process died *before*
    // `replace_file` made any change at all (e.g. right after the
    // `state.save()` call and before the call itself). No marker
    // exists (nothing ran yet) and the live file is still exactly the
    // pre-patch bytes recorded in `backup_hash`.
    let fx = build_fixture("replace-recover-swap-started-never-began");

    let mut state = TransactionState::load(&fx.backup_dir).unwrap();
    state.package_mut("p1.cpk").unwrap().stage = PackageStage::SwapStarted;
    state.save().unwrap();

    assert!(!pending_old_path(&fx.p1_path).exists());
    // Live file untouched -- still exactly the backed-up original.
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);

    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();

    // Nothing to roll back: the live file must remain byte-for-byte
    // identical to what it always was.
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);

    let recovered_state = TransactionState::load(&fx.backup_dir).unwrap();
    assert_eq!(recovered_state.outcome, TransactionOutcome::Failed);
    let p1_stage = recovered_state
        .packages
        .iter()
        .find(|p| p.target_package == "p1.cpk")
        .map(|p| p.stage);
    // Correctly reconciled back down to `TempBuilt` (never actually
    // swapped), not left dangling at `SwapStarted` and not
    // (incorrectly) advanced to `Swapped`/`RolledBack`.
    assert_eq!(p1_stage, Some(PackageStage::TempBuilt));
}

#[test]
fn committed_install_is_finalized_not_reverted_even_if_a_package_never_reached_swapped() {
    // "Committed" must win over any lingering per-package ambiguity:
    // once `TransactionState::outcome` is `Committed`, startup
    // recovery must finalize the journal entry as `Applied` and must
    // never revert any package, regardless of what any individual
    // package's stage looks like.
    let fx = build_fixture("replace-recover-committed-stays-finalized");

    let mut state = TransactionState::load(&fx.backup_dir).unwrap();
    state.package_mut("p1.cpk").unwrap().stage = PackageStage::Swapped;
    state.outcome = TransactionOutcome::Committed;
    state.save().unwrap();

    let patched_bytes = b"p1 fully committed patched bytes".to_vec();
    std::fs::write(&fx.p1_path, &patched_bytes).unwrap();

    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();

    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), patched_bytes);

    let journal_after = InstallationJournal::load_or_default(&fx.paths.journal_path).unwrap();
    assert_eq!(
        journal_after.entries()[0].status,
        asset_project::journal::InstallStatus::Applied
    );
}

#[test]
fn recovers_a_restore_interrupted_between_replace_steps_1_and_2() {
    let fx = build_fixture("replace-recover-restore-interrupted");

    // Reproduce a crash inside `replace_file` during a *restore*
    // (`restore_one_package`, stage still `Swapped` until it returns)
    // right after step 1 (the live/swapped file renamed aside to the
    // marker) but before step 2 (the backup content renamed into
    // place): the live file is momentarily missing entirely, and the
    // marker holds what used to be there (the swapped-in bytes).
    let mut state = TransactionState::load(&fx.backup_dir).unwrap();
    state.package_mut("p1.cpk").unwrap().stage = PackageStage::Swapped;
    state.save().unwrap();

    std::fs::write(
        pending_old_path(&fx.p1_path),
        b"p1 swapped (simulated pre-restore-crash state)",
    )
    .unwrap();
    let _ = std::fs::remove_file(&fx.p1_path);
    assert!(!fx.p1_path.exists());

    startup::recover_interrupted(&fx.env.game_root, fx.patch_id).unwrap();

    // Self-healing must first put the live file back exactly as it
    // was before this interrupted restore attempt (the swapped-in
    // bytes, per the marker), then `restore_swapped_packages` retries
    // the restore from scratch (the package is still `Swapped`) and
    // this time completes it cleanly.
    assert_eq!(std::fs::read(&fx.p1_path).unwrap(), fx.p1_original);
    assert!(
        !pending_old_path(&fx.p1_path).exists(),
        "stale replace marker must not survive recovery"
    );

    let recovered_state = TransactionState::load(&fx.backup_dir).unwrap();
    assert_eq!(recovered_state.outcome, TransactionOutcome::Failed);
    let p1_stage = recovered_state
        .packages
        .iter()
        .find(|p| p.target_package == "p1.cpk")
        .map(|p| p.stage);
    assert_eq!(p1_stage, Some(PackageStage::RolledBack));
}
