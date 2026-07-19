//! Integration-level validation scenarios exercised through the
//! public `transaction::validate_patch`/`transaction::apply` entry
//! points (as opposed to `validate.rs`'s own unit tests, which call
//! `validate::validate` directly against a hand-built manifest).

mod support;

use yaobow_asset_patcher::fixtures::FixtureChange;
use yaobow_asset_patcher::transaction::{self, ApplyOptions};

#[test]
fn apply_refuses_when_a_target_package_is_missing() {
    let env = support::TestEnv::new("validate-apply-missing-package");
    // No `scene.cpk` is ever written under `env.game_root` -- only the
    // PAL3 marker package exists.
    let patch_path = support::build_patch(
        &env,
        &[],
        vec![FixtureChange::add("scene.cpk", "new.dat", b"payload")],
    );

    let err = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("apply must refuse when a target package doesn't exist under the root");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));

    // Nothing should have been written to disk: no journal, no
    // backups directory.
    let paths = transaction::PatchPaths::for_root(&env.game_root);
    assert!(!paths.patch_state_dir.exists());
}

#[test]
fn apply_refuses_on_game_mismatch() {
    let env = support::TestEnv::new("validate-apply-game-mismatch");
    let (_p, hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash)],
        vec![FixtureChange::add("scene.cpk", "new.dat", b"payload")],
    );

    // The patch itself declares `target_game = "pal3"` (see
    // `support::build_patch`), so asking `apply()` to install it
    // against a `"pal4"` root must be rejected.
    let err = transaction::apply(&patch_path, &env.game_root, "pal4", ApplyOptions::default())
        .expect_err("apply must refuse a patch targeting a different game");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
}

#[test]
fn apply_refuses_on_fingerprint_mismatch() {
    let env = support::TestEnv::new("validate-apply-fingerprint-mismatch");
    let (_p, _hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);

    // Deliberately record the *wrong* expected fingerprint, simulating
    // a patch authored against a different (or since-modified) base
    // package.
    let wrong_hash = asset_project::hash::ContentHash::of(b"not what's actually on disk");
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", wrong_hash)],
        vec![FixtureChange::add("scene.cpk", "new.dat", b"payload")],
    );

    let err = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("apply must refuse on a package fingerprint mismatch");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
}

#[test]
fn apply_refuses_add_that_would_replace_an_existing_entry() {
    let env = support::TestEnv::new("validate-add-existing");
    let (_path, hash) = env.write_package("scene.cpk", &[("existing.dat", b"v1" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash)],
        vec![FixtureChange::add(
            "scene.cpk",
            "existing.dat",
            b"replacement",
        )],
    );

    let err = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("an Add change must not overwrite an existing entry");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
}

#[test]
fn apply_refuses_case_aliased_paths_within_one_patch() {
    let env = support::TestEnv::new("validate-intra-patch-alias");
    let (_, hash) = env.write_package("scene.cpk", &[("base.dat", b"base" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash)],
        vec![
            FixtureChange::add("scene.cpk", "mods/item.dat", b"first"),
            FixtureChange::add("SCENE.CPK", "MODS\\ITEM.DAT", b"first"),
        ],
    );

    let error = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("case aliases in one patch must be rejected");
    assert!(
        matches!(error, yaobow_asset_patcher::PatcherError::ValidationFailed(message)
            if message.contains("duplicate case-insensitive file paths"))
    );
}

#[test]
fn apply_refuses_case_aliases_for_the_same_physical_package() {
    let env = support::TestEnv::new("validate-package-case-alias");
    let (_path, hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash), ("SCENE.CPK", hash)],
        vec![
            FixtureChange::add("scene.cpk", "one.dat", b"one"),
            FixtureChange::add("SCENE.CPK", "two.dat", b"two"),
        ],
    );

    let err = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("case aliases resolving to one package must be rejected");
    assert!(matches!(
        err,
        yaobow_asset_patcher::PatcherError::ValidationFailed(_)
    ));
    let paths = transaction::PatchPaths::for_root(&env.game_root);
    assert!(!paths.patch_state_dir.exists());
}

#[cfg(unix)]
#[test]
fn apply_refuses_symlink_aliases_without_creating_pending_state() {
    use std::os::unix::fs::symlink;

    let env = support::TestEnv::new("validate-package-symlink-alias");
    let (scene_path, hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);
    symlink(&scene_path, env.game_root.join("alias.cpk")).unwrap();
    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash), ("alias.cpk", hash)],
        vec![
            FixtureChange::add("scene.cpk", "one.dat", b"one"),
            FixtureChange::add("alias.cpk", "two.dat", b"two"),
        ],
    );

    transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default())
        .expect_err("symlink aliases resolving to one package must be rejected");
    let paths = transaction::PatchPaths::for_root(&env.game_root);
    assert!(!paths.patch_state_dir.exists());
}

#[test]
fn validate_patch_reports_ok_for_a_matching_root_without_touching_anything() {
    let env = support::TestEnv::new("validate-apply-happy-path-precheck");
    let (path, hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);
    let before = std::fs::read(&path).unwrap();

    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash)],
        vec![FixtureChange::add("scene.cpk", "new.dat", b"payload")],
    );

    let (_manifest, summary) =
        transaction::validate_patch(&patch_path, &env.game_root, "pal3").unwrap();
    assert!(summary.is_ok(), "{summary:?}");

    // A pure validation pass must never modify the package.
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let paths = transaction::PatchPaths::for_root(&env.game_root);
    assert!(!paths.patch_state_dir.exists());
}

#[cfg(unix)]
#[test]
fn apply_refuses_when_target_package_is_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let env = support::TestEnv::new("validate-apply-read-only");
    let (path, hash) = env.write_package("scene.cpk", &[("a.dat", b"v1" as &[u8])]);

    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o444);
    std::fs::set_permissions(&path, perms).unwrap();

    let patch_path = support::build_patch(
        &env,
        &[("scene.cpk", hash)],
        vec![FixtureChange::add("scene.cpk", "new.dat", b"payload")],
    );

    let result = transaction::apply(&patch_path, &env.game_root, "pal3", ApplyOptions::default());

    // Restore write permission unconditionally so `TestEnv`'s `Drop`
    // (which removes the whole scratch directory) doesn't fail on a
    // still-read-only file, regardless of the assertion outcome below.
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o644);
    std::fs::set_permissions(&path, perms).unwrap();

    // Root is not expected to run this test suite, so a plain user's
    // 0o444 file must be genuinely unwritable and `apply()` must
    // refuse it during validation rather than fail midway through.
    if !nix_is_root() {
        let err = result.expect_err("apply must refuse a read-only target package");
        assert!(matches!(
            err,
            yaobow_asset_patcher::PatcherError::ValidationFailed(_)
        ));
    }
}

#[cfg(unix)]
fn nix_is_root() -> bool {
    // Avoid pulling in a `nix`/`libc` dependency just for this: `id -u`
    // is available on every unix CI runner and in every sandbox.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}
