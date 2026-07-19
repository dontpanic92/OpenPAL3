//! Happy-path integration tests: a multi-package `apply()` followed by
//! `rollback()`, exercising the real on-disk transaction engine (no
//! fault injection) end to end against synthetic PAL3-like roots.

mod support;

use asset_project::hash::ContentHash;
use uuid::Uuid;
use yaobow_asset_patcher::fault::{FailAt, FailurePoint};
use yaobow_asset_patcher::fixtures::FixtureChange;
use yaobow_asset_patcher::transaction::{self, ApplyOptions};

#[test]
fn apply_installs_multiple_packages_and_rollback_restores_them() {
    let env = support::TestEnv::new("apply-rollback-happy-path");

    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("original.dff", b"scene v1" as &[u8])]);
    let (other_path, other_hash) =
        env.write_package("other.cpk", &[("keep.tex", b"other v1" as &[u8])]);

    let original_entry_hash = ContentHash::of(b"other v1" as &[u8]);

    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash), ("other.cpk", other_hash)],
        vec![
            FixtureChange::add("scene.cpk", "new.dff", b"brand new scene asset"),
            FixtureChange::replace("other.cpk", "keep.tex", b"other v2", original_entry_hash),
        ],
    );

    let report = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect("apply should succeed against a freshly validated, matching root");

    assert_eq!(report.changes_applied, 2);
    let mut touched = report.touched_packages.clone();
    touched.sort();
    assert_eq!(
        touched,
        vec!["other.cpk".to_string(), "scene.cpk".to_string()]
    );

    // Both packages must reflect the patch's changes now.
    assert_eq!(
        support::read_cpk_entry(&scene_path, "original.dff"),
        b"scene v1"
    );
    assert_eq!(
        support::read_cpk_entry(&scene_path, "new.dff"),
        b"brand new scene asset"
    );
    assert_eq!(
        support::read_cpk_entry(&other_path, "keep.tex"),
        b"other v2"
    );

    // The journal should show exactly one Applied entry for this patch.
    let entries = transaction::list_journal_entries(&env.game_root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].patch_id, report.patch_id);
    assert_eq!(
        entries[0].status,
        asset_project::journal::InstallStatus::Applied
    );

    // Rolling back must restore both packages' pre-patch bytes exactly.
    let rollback_report = transaction::rollback(&env.game_root, report.patch_id).unwrap();
    let mut restored = rollback_report.packages_restored.clone();
    restored.sort();
    assert_eq!(
        restored,
        vec!["other.cpk".to_string(), "scene.cpk".to_string()]
    );

    assert_eq!(
        support::read_cpk_entry(&scene_path, "original.dff"),
        b"scene v1"
    );
    assert_eq!(
        support::read_cpk_entry(&other_path, "keep.tex"),
        b"other v1"
    );

    let entries = transaction::list_journal_entries(&env.game_root).unwrap();
    assert_eq!(
        entries[0].status,
        asset_project::journal::InstallStatus::RolledBack
    );
    let operations = yaobow_asset_patcher::manager::operations_dir(&env.game_root);
    assert!(
        !operations.exists() || std::fs::read_dir(operations).unwrap().next().is_none(),
        "successful uninstall must not retain recovery backups"
    );
}

#[test]
fn rollback_refuses_a_tampered_backup() {
    let env = support::TestEnv::new("apply-rollback-tampered-backup");

    let (_scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "new.dff", b"payload")],
    );

    let report = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect("apply should succeed");

    // Corrupt the backup that rollback would restore from. Look up its
    // actual recorded path via `TransactionState` rather than
    // recomputing it, since exactly how a `target_package` maps to a
    // backup file path is an internal (and collision-resistant, not
    // necessarily flat) implementation detail.
    let paths = transaction::PatchPaths::for_root(&env.game_root);
    let backup_dir = paths.backup_dir_for(report.patch_id);
    let state = yaobow_asset_patcher::state::TransactionState::load(&backup_dir).unwrap();
    let backup_file = state
        .packages
        .iter()
        .find(|p| p.target_package == "scene.cpk")
        .unwrap()
        .backup_path
        .clone();
    assert!(
        backup_file.exists(),
        "expected a backup file to have been written"
    );
    std::fs::write(&backup_file, b"corrupted bytes").unwrap();

    let err = transaction::rollback(&env.game_root, report.patch_id)
        .expect_err("rollback must refuse to restore from a hash-mismatched backup");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::CorruptBackup(_)
    ));
}

#[test]
fn rollback_of_unapplied_patch_is_rejected() {
    let env = support::TestEnv::new("apply-rollback-not-applied");
    let err = transaction::rollback(&env.game_root, Uuid::new_v4())
        .expect_err("rolling back a patch id never recorded in the journal must fail");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::PatchNotApplied(_)
    ));
}

#[test]
fn rollback_is_idempotent_when_called_twice() {
    let env = support::TestEnv::new("apply-rollback-idempotent");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "new.dff", b"payload")],
    );

    let report =
        transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    transaction::rollback(&env.game_root, report.patch_id).unwrap();
    // A second rollback call for the same (already rolled-back) patch
    // must not panic or corrupt anything further, even though the
    // journal no longer reports it `Applied`.
    let second = transaction::rollback(&env.game_root, report.patch_id);
    assert!(matches!(
        second,
        Err(yaobow_asset_patcher::PatcherError::PatchNotApplied(_))
    ));
    assert_eq!(support::read_cpk_entry(&scene_path, "a.dff"), b"scene v1");
}

#[test]
fn uninstall_prunes_parent_directories_created_by_added_files() {
    let env = support::TestEnv::new("uninstall-prunes-added-directories");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add(
            "scene.cpk",
            r"mods\nested\added.dat",
            b"added",
        )],
    );
    let report =
        transaction::apply(&patch, &env.game_root, "pal3", ApplyOptions::default()).unwrap();
    transaction::rollback(&env.game_root, report.patch_id).unwrap();

    let paths = support::cpk_paths(&scene_path);
    assert!(
        paths
            .iter()
            .all(|path| !path.to_lowercase().starts_with("mods"))
    );
}

#[test]
fn uninstall_of_an_older_disjoint_patch_preserves_a_newer_patch_in_the_same_package() {
    let env = support::TestEnv::new("apply-uninstall-disjoint-same-package");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);

    let patch_a_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add(
            "scene.cpk",
            "a_new.dff",
            b"patch a payload",
        )],
    );
    let report_a = transaction::apply(
        &patch_a_path,
        &env.game_root,
        "pal3",
        ApplyOptions::default(),
    )
    .expect("patch A should apply cleanly");

    // Both patches were authored against the same pristine package.
    // Manager-owned drift is accepted because the current package
    // matches the recorded head and the two internal file paths are
    // disjoint.
    let patch_b_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add(
            "scene.cpk",
            "b_new.dff",
            b"patch b payload",
        )],
    );
    let report_b = transaction::apply(
        &patch_b_path,
        &env.game_root,
        "pal3",
        ApplyOptions::default(),
    )
    .expect("patch B should apply cleanly on top of patch A");

    transaction::rollback(&env.game_root, report_a.patch_id)
        .expect("file-level uninstall must preserve disjoint newer files");
    assert!(support::try_read_cpk_entry(&scene_path, "a_new.dff").is_none());
    assert_eq!(
        support::read_cpk_entry(&scene_path, "b_new.dff"),
        b"patch b payload"
    );
    transaction::rollback(&env.game_root, report_b.patch_id)
        .expect("the newer patch must remain independently uninstallable");
    assert_eq!(support::read_cpk_entry(&scene_path, "a.dff"), b"scene v1");
}

#[test]
fn uninstalling_stacked_adds_prunes_a_directory_created_by_an_older_mod() {
    let env = support::TestEnv::new("apply-rollback-stacked-directory-prune");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_a = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "mods/a.dat", b"a")],
    );
    let report_a =
        transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();
    let patch_b = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "mods/b.dat", b"b")],
    );
    let report_b =
        transaction::apply(&patch_b, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    transaction::rollback(&env.game_root, report_a.patch_id).unwrap();
    transaction::rollback(&env.game_root, report_b.patch_id).unwrap();

    assert!(
        !support::cpk_paths(&scene_path)
            .iter()
            .any(|path| path.eq_ignore_ascii_case("mods")),
        "the last uninstall should prune the now-empty mod-created directory"
    );
}

#[test]
fn reinstall_preserves_stacked_directory_creation_provenance() {
    let env = support::TestEnv::new("apply-reinstall-directory-provenance");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_a = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "mods/a.dat", b"a")],
    );
    let report_a =
        transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();
    let patch_a = env.root.join("patch-a.ybpatch");
    std::fs::rename(env.root.join("update.ybpatch"), &patch_a).unwrap();
    let patch_b = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "mods/b.dat", b"b")],
    );
    let report_b =
        transaction::apply(&patch_b, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    transaction::rollback(&env.game_root, report_a.patch_id).unwrap();
    transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();
    transaction::rollback(&env.game_root, report_a.patch_id).unwrap();
    transaction::rollback(&env.game_root, report_b.patch_id).unwrap();

    assert!(
        !support::cpk_paths(&scene_path)
            .iter()
            .any(|path| path.eq_ignore_ascii_case("mods")),
        "reinstalling the directory-creating mod must not lose its original provenance"
    );
}

#[test]
fn install_rejects_an_exact_file_conflict() {
    let env = support::TestEnv::new("apply-file-conflict-rejected");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);
    let patch_a = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "shared.dff", b"patch a")],
    );
    transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    let patch_b = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "SHARED.DFF", b"patch b")],
    );
    let error = transaction::apply(&patch_b, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("case-insensitive exact file conflicts must be rejected");
    assert!(matches!(
        error,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
    assert_eq!(
        support::read_cpk_entry(&scene_path, "shared.dff"),
        b"patch a"
    );
}

#[test]
fn install_rejects_unmanaged_package_drift() {
    let env = support::TestEnv::new("apply-unmanaged-drift-rejected");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_a = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "managed.dat", b"managed")],
    );
    transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    let externally_modified = env.root.join("externally-modified.cpk");
    packfs::cpk::CpkRebuilder::rebuild(
        &scene_path,
        &externally_modified,
        &[packfs::cpk::CpkEdit::file(
            "external.dat",
            b"external".to_vec(),
        )],
    )
    .unwrap();
    std::fs::copy(&externally_modified, &scene_path).unwrap();

    let patch_b = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "second.dat", b"second")],
    );
    let error = transaction::apply(&patch_b, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("unmanaged changes must invalidate the recorded package head");
    assert!(matches!(
        error,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
}

#[test]
fn rollback_of_non_overlapping_patches_is_never_blocked() {
    let env = support::TestEnv::new("apply-rollback-no-overlap-not-blocked");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);
    let (other_path, other_hash) =
        env.write_package("other.cpk", &[("b.tex", b"other v1" as &[u8])]);

    let patch_a_path = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add(
            "scene.cpk",
            "a_new.dff",
            b"patch a payload",
        )],
    );
    let report_a = transaction::apply(
        &patch_a_path,
        &env.game_root,
        "pal3",
        ApplyOptions::default(),
    )
    .expect("patch A should apply cleanly");

    let patch_b_path = support::build_patch(
        &env,
        &[("other.cpk", other_hash)],
        vec![FixtureChange::add(
            "other.cpk",
            "b_new.tex",
            b"patch b payload",
        )],
    );
    let report_b = transaction::apply(
        &patch_b_path,
        &env.game_root,
        "pal3",
        ApplyOptions::default(),
    )
    .expect("patch B should apply cleanly");

    // A and B touch entirely disjoint packages, so B being a newer
    // `Applied` patch must not block rolling A back.
    transaction::rollback(&env.game_root, report_a.patch_id)
        .expect("rollback of a non-overlapping older patch must not be blocked");

    assert_eq!(support::read_cpk_entry(&scene_path, "a.dff"), b"scene v1");
    assert_eq!(
        support::read_cpk_entry(&other_path, "b_new.tex"),
        b"patch b payload"
    );
    let _ = report_b;
}

#[test]
fn uninstall_does_not_need_an_unrelated_newer_patch_install_state() {
    let env = support::TestEnv::new("apply-rollback-missing-newer-state");
    let (scene_path, scene_hash) =
        env.write_package("scene.cpk", &[("a.dff", b"scene v1" as &[u8])]);
    let patch_a = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "a_new.dff", b"patch a")],
    );
    let report_a =
        transaction::apply(&patch_a, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    let patch_b = support::build_patch(
        &env,
        &[("scene.cpk", scene_hash)],
        vec![FixtureChange::add("scene.cpk", "b_new.dff", b"patch b")],
    );
    let report_b =
        transaction::apply(&patch_b, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    let newer_state = transaction::PatchPaths::for_root(&env.game_root)
        .backup_dir_for(report_b.patch_id)
        .join(yaobow_asset_patcher::state::TRANSACTION_STATE_FILE_NAME);
    std::fs::remove_file(newer_state).unwrap();

    transaction::rollback(&env.game_root, report_a.patch_id)
        .expect("file-level uninstall only needs the selected patch's original backup");
    assert_eq!(
        support::read_cpk_entry(&scene_path, "b_new.dff"),
        b"patch b"
    );
}

#[test]
fn backup_paths_for_colliding_package_names_do_not_collide() {
    // `a/b.cpk` (nested) and `a__b.cpk` (flat, containing the literal
    // separator-replacement string the old sanitizer used) used to
    // map to the exact same sanitized backup filename, so backing up
    // one silently clobbered the other's backup. Apply a single patch
    // touching both in the same transaction and confirm both round
    // trip independently.
    let env = support::TestEnv::new("apply-rollback-backup-collision");
    let (nested_path, nested_hash) =
        env.write_package("a/b.cpk", &[("orig.dat", b"nested a/b v1" as &[u8])]);
    let (flat_path, flat_hash) =
        env.write_package("a__b.cpk", &[("orig.dat", b"flat a__b v1" as &[u8])]);
    let nested_original_bytes = std::fs::read(&nested_path).unwrap();
    let flat_original_bytes = std::fs::read(&flat_path).unwrap();

    let patch_path = support::build_patch(
        &env,
        &[("a/b.cpk", nested_hash), ("a__b.cpk", flat_hash)],
        vec![
            FixtureChange::add("a/b.cpk", "new.dat", b"nested patched"),
            FixtureChange::add("a__b.cpk", "new.dat", b"flat patched"),
        ],
    );

    let report = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect("patch touching both colliding package names should apply cleanly");

    assert_eq!(
        support::read_cpk_entry(&nested_path, "new.dat"),
        b"nested patched"
    );
    assert_eq!(
        support::read_cpk_entry(&flat_path, "new.dat"),
        b"flat patched"
    );

    // Confirm the two packages actually got distinct backup paths (the
    // structural fix), not just that both happen to still work.
    let backup_dir =
        transaction::PatchPaths::for_root(&env.game_root).backup_dir_for(report.patch_id);
    let state = yaobow_asset_patcher::state::TransactionState::load(&backup_dir).unwrap();
    let nested_backup = state
        .packages
        .iter()
        .find(|p| p.target_package == "a/b.cpk")
        .unwrap()
        .backup_path
        .clone();
    let flat_backup = state
        .packages
        .iter()
        .find(|p| p.target_package == "a__b.cpk")
        .unwrap()
        .backup_path
        .clone();
    assert_ne!(
        nested_backup, flat_backup,
        "colliding target_package names must map to distinct backup paths"
    );
    assert_eq!(
        std::fs::read(&nested_backup).unwrap(),
        nested_original_bytes
    );
    assert_eq!(std::fs::read(&flat_backup).unwrap(), flat_original_bytes);

    transaction::rollback(&env.game_root, report.patch_id)
        .expect("rollback of both colliding packages should restore each independently");

    assert_eq!(std::fs::read(&nested_path).unwrap(), nested_original_bytes);
    assert_eq!(std::fs::read(&flat_path).unwrap(), flat_original_bytes);
}

// Sanity check that `FailAt`/`FailurePoint` values compile and compare
// as expected outside of `fault.rs`'s own unit tests -- exercised more
// thoroughly in `failure_injection.rs`.
#[test]
fn fail_at_matches_only_its_own_point() {
    use asset_project::manifest::TargetPackage;
    use yaobow_asset_patcher::fault::FaultInjector;

    let p1 = TargetPackage::new("scene.cpk").unwrap();
    let p2 = TargetPackage::new("other.cpk").unwrap();
    let injector = FailAt(FailurePoint::AfterSwap(p1.clone()));
    assert!(injector.should_fail(&FailurePoint::AfterSwap(p1)));
    assert!(!injector.should_fail(&FailurePoint::AfterSwap(p2)));
}
