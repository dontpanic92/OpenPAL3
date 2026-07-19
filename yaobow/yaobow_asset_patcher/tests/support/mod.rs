//! Shared test-support helpers for the integration tests in this
//! directory. Each `tests/*.rs` file is compiled as its own separate
//! crate, so this lives under `tests/support/mod.rs` (not
//! `tests/support.rs`) specifically so it is *not* itself picked up as
//! a standalone test binary — only `mod support;`-including files are.
//!
//! Everything here builds on the `test-support`-gated fixture builders
//! re-exported by the library itself (`yaobow_asset_patcher::fixtures`,
//! `::test_scratch`), reached the same way any other external
//! integration-test crate would: as a regular dependency (see this
//! crate's own `[dev-dependencies]` self-reference with
//! `features = ["test-support"]`).
//!
//! Since `support` is compiled separately (and with a fresh set of
//! `#[warn(dead_code)]` lints) for each `tests/*.rs` binary that
//! includes it via `mod support;`, and no single test file uses every
//! helper here, dead-code warnings are suppressed crate-wide rather
//! than per-item.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use asset_project::hash::ContentHash;
use yaobow_asset_patcher::fixtures::{self, FixtureChange};

/// A scratch PAL3-like install root: a real (synthetic) marker
/// `basedata/basedata.cpk` plus whatever additional packages a test
/// adds via [`TestEnv::write_package`]. Cleans itself up on drop.
pub struct TestEnv {
    pub root: PathBuf,
    pub game_root: PathBuf,
}

impl TestEnv {
    pub fn new(name: &str) -> Self {
        let root = yaobow_asset_patcher::test_scratch::dir(name);
        let game_root = root.join("game");
        fixtures::write_fixture_cpk(
            &game_root.join("basedata"),
            "basedata.cpk",
            &[("marker.txt", b"pal3 marker")],
        );
        Self { root, game_root }
    }

    /// Writes a synthetic (but structurally valid) `.cpk` package at
    /// `target_package` (forward-slash separated, e.g. `"scene.cpk"`
    /// or `"sub/other.cpk"`) under this env's game root, containing
    /// `files` (backslash-separated internal paths -> content). Returns
    /// its physical path and whole-file [`ContentHash`] (the same
    /// fingerprinting convention `crate::fingerprint` uses), so callers
    /// can build a matching `PackageFingerprint` for a `.ybpatch`.
    pub fn write_package(
        &self,
        target_package: &str,
        files: &[(&str, &[u8])],
    ) -> (PathBuf, ContentHash) {
        let relative = Path::new(target_package);
        let dir = self
            .game_root
            .join(relative.parent().unwrap_or(Path::new("")));
        let name = relative.file_name().unwrap().to_str().unwrap();
        let path = fixtures::write_fixture_cpk(&dir, name, files);
        let hash = whole_file_hash(&path);
        (path, hash)
    }

    pub fn read_package_bytes(&self, target_package: &str) -> Vec<u8> {
        std::fs::read(self.game_root.join(target_package)).unwrap()
    }

    pub fn package_path(&self, target_package: &str) -> PathBuf {
        self.game_root.join(target_package)
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn whole_file_hash(path: &Path) -> ContentHash {
    ContentHash::of(&std::fs::read(path).unwrap())
}

/// Builds a `.ybpatch` at `env.root.join("update.ybpatch")` covering
/// `fingerprints` (target_package -> expected pre-patch whole-file
/// hash) and `changes`, and returns its path.
pub fn build_patch(
    env: &TestEnv,
    fingerprints: &[(&str, ContentHash)],
    changes: Vec<FixtureChange>,
) -> PathBuf {
    let path = env.root.join("update.ybpatch");
    fixtures::build_fixture_ybpatch(&path, "pal3", 1, fingerprints, &changes);
    path
}

/// Reads back a single entry's content from a (real, structurally
/// valid) `.cpk` file, for asserting on post-apply/-rollback package
/// contents. `internal_path` uses backslash separators, matching the
/// convention `fixtures::build_fixture_cpk` and `AssetChange` both
/// use internally.
pub fn read_cpk_entry(path: &Path, internal_path: &str) -> Vec<u8> {
    try_read_cpk_entry(path, internal_path).unwrap()
}

pub fn try_read_cpk_entry(path: &Path, internal_path: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).unwrap();
    let mut archive =
        packfs::cpk::CpkArchive::load(Box::new(std::io::BufReader::new(file))).unwrap();
    let mut entry = archive.open_str(internal_path).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).unwrap();
    Some(buf)
}

pub fn cpk_paths(path: &Path) -> Vec<String> {
    let file = std::fs::File::open(path).unwrap();
    let mut archive =
        packfs::cpk::CpkArchive::load(Box::new(std::io::BufReader::new(file))).unwrap();
    archive.full_paths().unwrap()
}
