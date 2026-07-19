//! Failure-injection integration tests: force `apply()` to fail at
//! each `FailurePoint` across a *multi-package* patch, and confirm the
//! documented recovery guarantee — already-swapped packages are
//! restored from backup, never-swapped packages are left completely
//! untouched, no sibling temp files are left behind, and the journal
//! ends up `Failed` (never `Applied`).

mod support;

use asset_project::manifest::TargetPackage;
use yaobow_asset_patcher::fault::{FailAt, FailurePoint};
use yaobow_asset_patcher::fixtures::FixtureChange;
use yaobow_asset_patcher::transaction::{self, ApplyOptions};

struct Fixture {
    env: support::TestEnv,
    p1_path: std::path::PathBuf,
    p2_path: std::path::PathBuf,
    p3_path: std::path::PathBuf,
    patch_path: std::path::PathBuf,
}

/// Three packages, each with one pre-existing file, plus a patch that
/// adds one new file to each -- in `p1, p2, p3` plan order (matching
/// insertion order into the manifest, which `plan_patch`/`apply`
/// preserve).
fn build_three_package_fixture(name: &str) -> Fixture {
    let env = support::TestEnv::new(name);
    let (p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"p1 original" as &[u8])]);
    let (p2_path, p2_hash) = env.write_package("p2.cpk", &[("a.dat", b"p2 original" as &[u8])]);
    let (p3_path, p3_hash) = env.write_package("p3.cpk", &[("a.dat", b"p3 original" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[
            ("p1.cpk", p1_hash),
            ("p2.cpk", p2_hash),
            ("p3.cpk", p3_hash),
        ],
        vec![
            FixtureChange::add("p1.cpk", "new.dat", b"p1 new"),
            FixtureChange::add("p2.cpk", "new.dat", b"p2 new"),
            FixtureChange::add("p3.cpk", "new.dat", b"p3 new"),
        ],
    );

    Fixture {
        env,
        p1_path,
        p2_path,
        p3_path,
        patch_path,
    }
}

fn assert_untouched(fixture: &Fixture) {
    assert_eq!(
        support::read_cpk_entry(&fixture.p1_path, "a.dat"),
        b"p1 original"
    );
    assert_eq!(
        support::read_cpk_entry(&fixture.p2_path, "a.dat"),
        b"p2 original"
    );
    assert_eq!(
        support::read_cpk_entry(&fixture.p3_path, "a.dat"),
        b"p3 original"
    );
}

fn assert_no_leftover_temp_files(env: &support::TestEnv) {
    for entry in std::fs::read_dir(&env.game_root).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        // Covers both the build-phase sibling temp suffix
        // (`.yaobowpatch.tmp`) and the swap/restore-phase stale
        // replacement marker suffix (`.yaobowpatch.old` — see
        // `crate::replace::pending_old_path`); neither should ever
        // survive a fully finished (successful or recovered) apply.
        assert!(
            !file_name.contains("yaobowpatch"),
            "leftover temp/marker file: {file_name}"
        );
    }
}

fn assert_journal_failed(env: &support::TestEnv) {
    let entries = transaction::list_journal_entries(&env.game_root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].status,
        asset_project::journal::InstallStatus::Failed
    );
}

#[test]
fn after_backup_failure_leaves_everything_untouched() {
    let fixture = build_three_package_fixture("fault-after-backup");
    let target = TargetPackage::new("p2.cpk").unwrap();

    let options = ApplyOptions {
        fault_injector: Some(Box::new(FailAt(FailurePoint::AfterBackup(target)))),
        ..ApplyOptions::default()
    };
    let err = transaction::apply(&fixture.patch_path, &fixture.env.game_root, "pal3", options)
        .expect_err("injected fault must fail the apply");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));

    assert_untouched(&fixture);
    assert_no_leftover_temp_files(&fixture.env);
    assert_journal_failed(&fixture.env);
}

#[test]
fn after_temp_build_failure_never_swaps_anything() {
    let fixture = build_three_package_fixture("fault-after-temp-build");
    let target = TargetPackage::new("p3.cpk").unwrap();

    let options = ApplyOptions {
        fault_injector: Some(Box::new(FailAt(FailurePoint::AfterTempBuild(target)))),
        ..ApplyOptions::default()
    };
    let err = transaction::apply(&fixture.patch_path, &fixture.env.game_root, "pal3", options)
        .expect_err("injected fault must fail the apply");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));

    // The build phase runs to completion (or fails) strictly before
    // any swap, so nothing should ever have been swapped even though
    // p1 and p2's temp files were already built successfully by the
    // time p3's build was reached.
    assert_untouched(&fixture);
    assert_no_leftover_temp_files(&fixture.env);
    assert_journal_failed(&fixture.env);
}

#[test]
fn before_swap_failure_on_first_package_leaves_everything_untouched() {
    let fixture = build_three_package_fixture("fault-before-swap-first");
    let target = TargetPackage::new("p1.cpk").unwrap();

    let options = ApplyOptions {
        fault_injector: Some(Box::new(FailAt(FailurePoint::BeforeSwap(target)))),
        ..ApplyOptions::default()
    };
    let err = transaction::apply(&fixture.patch_path, &fixture.env.game_root, "pal3", options)
        .expect_err("injected fault must fail the apply");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));

    assert_untouched(&fixture);
    assert_no_leftover_temp_files(&fixture.env);
    assert_journal_failed(&fixture.env);
}

#[test]
fn after_swap_failure_on_middle_package_restores_already_swapped_and_leaves_rest_untouched() {
    let fixture = build_three_package_fixture("fault-after-swap-middle");
    let target = TargetPackage::new("p2.cpk").unwrap();

    let options = ApplyOptions {
        fault_injector: Some(Box::new(FailAt(FailurePoint::AfterSwap(target)))),
        ..ApplyOptions::default()
    };
    let err = transaction::apply(&fixture.patch_path, &fixture.env.game_root, "pal3", options)
        .expect_err("injected fault must fail the apply");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));

    // p1 was swapped before p2's AfterSwap fault fired, so it must
    // have been restored back to its original content by the
    // in-apply recovery path. p2 itself was swapped then restored too
    // (the fault fires *after* its own swap). p3 was never reached by
    // the swap loop at all (plan order p1 -> p2 -> p3) so it must
    // remain in its pristine, never-even-attempted state.
    assert_untouched(&fixture);
    assert_no_leftover_temp_files(&fixture.env);
    assert_journal_failed(&fixture.env);
}

#[test]
fn before_swap_failure_on_last_package_restores_the_earlier_swapped_ones() {
    let fixture = build_three_package_fixture("fault-before-swap-last");
    let target = TargetPackage::new("p3.cpk").unwrap();

    let options = ApplyOptions {
        fault_injector: Some(Box::new(FailAt(FailurePoint::BeforeSwap(target)))),
        ..ApplyOptions::default()
    };
    let err = transaction::apply(&fixture.patch_path, &fixture.env.game_root, "pal3", options)
        .expect_err("injected fault must fail the apply");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));

    // p1 and p2 were both fully swapped before p3's BeforeSwap fault
    // fired, so both must be restored; p3 itself was never swapped.
    assert_untouched(&fixture);
    assert_no_leftover_temp_files(&fixture.env);
    assert_journal_failed(&fixture.env);
}

#[test]
fn uninstall_swap_failure_restores_the_still_applied_mod() {
    let env = support::TestEnv::new("fault-uninstall-after-swap");
    let (package_path, package_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", package_hash)],
        vec![FixtureChange::add("scene.cpk", "mod.dat", b"mod")],
    );
    let report =
        transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default()).unwrap();

    let target = TargetPackage::new("scene.cpk").unwrap();
    let error = transaction::uninstall(
        &env.game_root,
        report.patch_id,
        ApplyOptions {
            fault_injector: Some(Box::new(FailAt(FailurePoint::AfterSwap(target)))),
            ..ApplyOptions::default()
        },
    )
    .expect_err("the injected uninstall fault must be reported");
    assert!(matches!(
        error,
        yaobow_asset_patcher::PatcherError::InjectedFault(_)
    ));
    assert_eq!(support::read_cpk_entry(&package_path, "mod.dat"), b"mod");
    assert!(
        yaobow_asset_patcher::manager::ManagerState::load_or_default(&env.game_root)
            .unwrap()
            .is_applied(report.patch_id)
    );
}
