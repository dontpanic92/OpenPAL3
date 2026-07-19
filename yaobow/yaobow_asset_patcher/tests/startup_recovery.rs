//! Startup pending-journal detection and interrupted-install recovery.
//!
//! `apply()` itself always self-heals synchronously before returning
//! an error (see `failure_injection.rs`), so exercising the *startup*
//! recovery path realistically means simulating what the on-disk state
//! looks like after an actual process crash -- i.e. a `Pending` journal
//! entry plus a `TransactionState` with some packages already swapped
//! and others not, constructed directly (bypassing `apply()` entirely)
//! rather than injected through a fault point.

mod support;

use asset_project::journal::{InstallStatus, InstallationJournal};
use yaobow_asset_patcher::startup;
use yaobow_asset_patcher::state::{PackageStage, TransactionOutcome, TransactionState};
use yaobow_asset_patcher::transaction::PatchPaths;

#[test]
fn recovers_a_simulated_crash_mid_swap_phase() {
    let env = support::TestEnv::new("startup-recover-mid-swap");
    let (p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"p1 original" as &[u8])]);
    let (p2_path, p2_hash) = env.write_package("p2.cpk", &[("a.dat", b"p2 original" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[("p1.cpk", p1_hash), ("p2.cpk", p2_hash)],
        vec![
            yaobow_asset_patcher::fixtures::FixtureChange::add("p1.cpk", "new.dat", b"p1 new"),
            yaobow_asset_patcher::fixtures::FixtureChange::add("p2.cpk", "new.dat", b"p2 new"),
        ],
    );
    let manifest = asset_project::patch::YbpatchReader::open(&patch_path)
        .unwrap()
        .manifest()
        .clone();
    let patch_id = manifest.patch_id;

    let paths = PatchPaths::for_root(&env.game_root);
    std::fs::create_dir_all(&paths.patch_state_dir).unwrap();
    let backup_dir = paths.backup_dir_for(patch_id);
    std::fs::create_dir_all(&backup_dir).unwrap();

    let target_packages = vec![
        ("p1.cpk".to_string(), p1_path.clone()),
        ("p2.cpk".to_string(), p2_path.clone()),
    ];
    let mut state = TransactionState::new(
        patch_id,
        &patch_path,
        &env.game_root,
        &backup_dir,
        &target_packages,
    );

    // Back up both packages' pre-patch bytes, exactly as `apply()`'s
    // backup phase would have.
    let p1_original = std::fs::read(&p1_path).unwrap();
    let p2_original = std::fs::read(&p2_path).unwrap();
    {
        let pkg = state.package_mut("p1.cpk").unwrap();
        std::fs::create_dir_all(pkg.backup_path.parent().unwrap()).unwrap();
        std::fs::write(&pkg.backup_path, &p1_original).unwrap();
        pkg.backup_hash = Some(support::whole_file_hash(&pkg.backup_path));
        pkg.stage = PackageStage::Swapped;
    }
    {
        let pkg = state.package_mut("p2.cpk").unwrap();
        std::fs::create_dir_all(pkg.backup_path.parent().unwrap()).unwrap();
        std::fs::write(&pkg.backup_path, &p2_original).unwrap();
        pkg.backup_hash = Some(support::whole_file_hash(&pkg.backup_path));
        let temp_path = p2_path.with_extension("cpk.yaobowpatch.tmp");
        std::fs::write(&temp_path, b"orphaned rebuilt package").unwrap();
        pkg.temp_path = Some(temp_path);
        pkg.stage = PackageStage::TempBuilt;
    }
    state.save().unwrap();

    // Simulate that p1's swap actually happened (its live file now has
    // post-patch content) while p2's swap never started (still its
    // original bytes) -- the process "crashed" right here, before
    // `apply()`'s own recovery ever ran.
    std::fs::write(&p1_path, b"p1 swapped (simulated post-crash state)").unwrap();

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

    // --- Startup detection ---
    let pending = startup::detect_pending(&env.game_root).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].patch_id, patch_id);
    assert!(
        pending[0].state.is_some(),
        "expected a TransactionState to have been found"
    );

    // --- Recovery ---
    startup::recover_interrupted(&env.game_root, patch_id).unwrap();

    // p1 must be restored back to its pre-patch bytes; p2 (never
    // swapped) must remain exactly as it was, untouched by recovery.
    assert_eq!(std::fs::read(&p1_path).unwrap(), p1_original);
    assert_eq!(std::fs::read(&p2_path).unwrap(), p2_original);

    let recovered_state = TransactionState::load(&backup_dir).unwrap();
    assert_eq!(recovered_state.outcome, TransactionOutcome::Failed);
    let stage_of = |pkg: &str| {
        recovered_state
            .packages
            .iter()
            .find(|p| p.target_package == pkg)
            .map(|p| p.stage)
    };
    assert_eq!(stage_of("p1.cpk"), Some(PackageStage::RolledBack));
    // p2 was never `Swapped`, so `restore_swapped_packages` correctly
    // leaves its stage alone (still `TempBuilt`) -- there was nothing
    // to roll back.
    assert_eq!(stage_of("p2.cpk"), Some(PackageStage::TempBuilt));
    let p2_state = recovered_state
        .packages
        .iter()
        .find(|package| package.target_package == "p2.cpk")
        .unwrap();
    assert!(
        !p2_state.temp_path.as_ref().unwrap().exists(),
        "startup recovery must remove unswapped rebuilt packages"
    );

    let journal_after = InstallationJournal::load_or_default(&paths.journal_path).unwrap();
    assert_eq!(journal_after.entries()[0].status, InstallStatus::Failed);

    let pending_after = startup::detect_pending(&env.game_root).unwrap();
    assert!(
        pending_after.is_empty(),
        "recovered install must no longer be pending"
    );
}

#[test]
fn recover_interrupted_is_idempotent() {
    let env = support::TestEnv::new("startup-recover-idempotent");
    let (p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"p1 original" as &[u8])]);

    let patch_path = support::build_patch(
        &env,
        &[("p1.cpk", p1_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "p1.cpk", "new.dat", b"p1 new",
        )],
    );
    let manifest = asset_project::patch::YbpatchReader::open(&patch_path)
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
        pkg.stage = PackageStage::Swapped;
    }
    state.save().unwrap();
    std::fs::write(&p1_path, b"p1 swapped (simulated post-crash state)").unwrap();

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

    startup::recover_interrupted(&env.game_root, patch_id).unwrap();
    assert_eq!(std::fs::read(&p1_path).unwrap(), p1_original);

    // Calling it again (e.g. the GUI re-scans on every launch) must
    // not error or re-corrupt anything -- the package is already back
    // to its original bytes and already `RolledBack`.
    startup::recover_interrupted(&env.game_root, patch_id).unwrap();
    assert_eq!(std::fs::read(&p1_path).unwrap(), p1_original);
}

#[test]
fn detect_pending_ignores_completed_installs() {
    let env = support::TestEnv::new("startup-detect-ignores-completed");
    let (_p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"p1 original" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("p1.cpk", p1_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "p1.cpk", "new.dat", b"p1 new",
        )],
    );

    let report = yaobow_asset_patcher::transaction::apply(
        &patch_path,
        &env.game_root,
        "pal3",
        yaobow_asset_patcher::transaction::ApplyOptions::default(),
    )
    .unwrap();

    let pending = startup::detect_pending(&env.game_root).unwrap();
    assert!(
        pending.is_empty(),
        "a successfully applied (Applied) journal entry must not show up as pending"
    );
    let _ = report;
}

#[test]
fn finalizes_committed_state_without_reverting_packages() {
    let env = support::TestEnv::new("startup-finalize-committed");
    let (p1_path, p1_hash) = env.write_package("p1.cpk", &[("a.dat", b"original" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("p1.cpk", p1_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "p1.cpk", "new.dat", b"new",
        )],
    );
    let manifest = asset_project::patch::YbpatchReader::open(&patch_path)
        .unwrap()
        .manifest()
        .clone();
    let paths = PatchPaths::for_root(&env.game_root);
    let backup_dir = paths.backup_dir_for(manifest.patch_id);
    std::fs::create_dir_all(&backup_dir).unwrap();

    let original = std::fs::read(&p1_path).unwrap();
    let patched = b"fully committed package bytes".to_vec();
    std::fs::write(&p1_path, &patched).unwrap();

    let mut state = TransactionState::new(
        manifest.patch_id,
        &patch_path,
        &env.game_root,
        &backup_dir,
        &[("p1.cpk".to_string(), p1_path.clone())],
    );
    {
        let pkg = state.package_mut("p1.cpk").unwrap();
        std::fs::create_dir_all(pkg.backup_path.parent().unwrap()).unwrap();
        std::fs::write(&pkg.backup_path, original).unwrap();
        pkg.stage = PackageStage::Swapped;
    }
    state.outcome = TransactionOutcome::Committed;
    state.save().unwrap();

    let mut journal = InstallationJournal::new();
    journal
        .begin(
            manifest.patch_id,
            &patch_path,
            asset_project::ContentHash::of(&serde_json::to_vec(&manifest).unwrap()),
            manifest.base_project_version,
        )
        .unwrap();
    journal.save(&paths.journal_path).unwrap();

    startup::recover_interrupted(&env.game_root, manifest.patch_id).unwrap();

    assert_eq!(std::fs::read(&p1_path).unwrap(), patched);
    let journal = InstallationJournal::load(&paths.journal_path).unwrap();
    assert_eq!(journal.entries()[0].status, InstallStatus::Applied);
}

#[test]
fn retries_a_failed_partial_uninstall_back_to_applied() {
    let env = support::TestEnv::new("startup-recover-uninstall");
    let (package_path, package_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", package_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "scene.cpk",
            "mod.dat",
            b"mod",
        )],
    );
    let report = yaobow_asset_patcher::transaction::apply(
        &patch_path,
        &env.game_root,
        "pal3",
        yaobow_asset_patcher::transaction::ApplyOptions::default(),
    )
    .unwrap();
    let applied_bytes = std::fs::read(&package_path).unwrap();
    let install_state = TransactionState::load(
        PatchPaths::for_root(&env.game_root).backup_dir_for(report.patch_id),
    )
    .unwrap();
    let original_bytes = std::fs::read(&install_state.packages[0].backup_path).unwrap();

    let operation_id = uuid::Uuid::new_v4();
    let operation_dir = yaobow_asset_patcher::manager::operation_dir(&env.game_root, operation_id);
    std::fs::create_dir_all(&operation_dir).unwrap();
    let mut state = TransactionState::new_uninstall(
        report.patch_id,
        yaobow_asset_patcher::manager::managed_patch_path(&env.game_root, report.patch_id),
        &env.game_root,
        &operation_dir,
        &[("scene.cpk".to_string(), package_path.clone())],
    );
    {
        let package = state.package_mut("scene.cpk").unwrap();
        std::fs::create_dir_all(package.backup_path.parent().unwrap()).unwrap();
        std::fs::write(&package.backup_path, &applied_bytes).unwrap();
        package.backup_hash = Some(support::whole_file_hash(&package.backup_path));
        package.stage = PackageStage::Swapped;
    }
    state.outcome = TransactionOutcome::Failed;
    state.error = Some("simulated failed restore".to_string());
    state.save().unwrap();
    std::fs::write(&package_path, original_bytes).unwrap();

    assert_eq!(
        startup::detect_pending_uninstalls(&env.game_root)
            .unwrap()
            .len(),
        1
    );
    startup::recover_interrupted_uninstall(&env.game_root, operation_id).unwrap();

    assert_eq!(std::fs::read(&package_path).unwrap(), applied_bytes);
    assert_eq!(
        TransactionState::load(&operation_dir).unwrap().outcome,
        TransactionOutcome::Failed
    );
    assert!(
        yaobow_asset_patcher::manager::ManagerState::load_or_default(&env.game_root)
            .unwrap()
            .is_applied(report.patch_id)
    );
}

#[test]
fn finalizes_a_committed_uninstall_after_a_crash() {
    let env = support::TestEnv::new("startup-finalize-uninstall");
    let (package_path, package_hash) =
        env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", package_hash)],
        vec![yaobow_asset_patcher::fixtures::FixtureChange::add(
            "scene.cpk",
            "mod.dat",
            b"mod",
        )],
    );
    let report = yaobow_asset_patcher::transaction::apply(
        &patch_path,
        &env.game_root,
        "pal3",
        yaobow_asset_patcher::transaction::ApplyOptions::default(),
    )
    .unwrap();
    let install_state = TransactionState::load(
        PatchPaths::for_root(&env.game_root).backup_dir_for(report.patch_id),
    )
    .unwrap();
    let original_bytes = std::fs::read(&install_state.packages[0].backup_path).unwrap();
    std::fs::write(&package_path, original_bytes).unwrap();

    let operation_id = uuid::Uuid::new_v4();
    let operation_dir = yaobow_asset_patcher::manager::operation_dir(&env.game_root, operation_id);
    std::fs::create_dir_all(&operation_dir).unwrap();
    let mut state = TransactionState::new_uninstall(
        report.patch_id,
        yaobow_asset_patcher::manager::managed_patch_path(&env.game_root, report.patch_id),
        &env.game_root,
        &operation_dir,
        &[("scene.cpk".to_string(), package_path.clone())],
    );
    {
        let package = state.package_mut("scene.cpk").unwrap();
        package.stage = PackageStage::Swapped;
        package.installed_hash = Some(support::whole_file_hash(&package_path));
    }
    state.outcome = TransactionOutcome::RolledBack;
    state.save().unwrap();

    startup::recover_interrupted_uninstall(&env.game_root, operation_id).unwrap();

    assert!(
        !yaobow_asset_patcher::manager::ManagerState::load_or_default(&env.game_root)
            .unwrap()
            .is_applied(report.patch_id)
    );
    let journal =
        InstallationJournal::load(PatchPaths::for_root(&env.game_root).journal_path).unwrap();
    assert_eq!(journal.entries()[0].status, InstallStatus::RolledBack);
}
