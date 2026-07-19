//! PAL3 install-root detection and package lookup.
//!
//! Built directly on [`packfs::init_virtual_fs_with_catalog`]'s
//! [`packfs::AssetCatalog`] — the same provenance catalog a real PAL3
//! runtime builds when mounting its virtual file system — so "is this
//! a PAL3 root" and "does target package X exist" both ask the exact
//! question the game itself would ask, rather than a parallel
//! heuristic that could drift from actual mount behavior.

use std::path::{Path, PathBuf};

use packfs::{AssetCatalog, PackageType};

/// Marker package every known PAL3 install has: `basedata/basedata.cpk`,
/// mounted at the fixed VFS path `/basedata/basedata`
/// (`shared::openpal3::AssetManager::basedata_path`). Used both to
/// detect a candidate root and as the "root looks right" signal in
/// [`RootValidation`].
const PAL3_MARKER_VFS_MOUNT: &str = "/basedata/basedata";

/// A candidate PAL3 install root, with its package catalog already
/// built (mounting is a full directory walk, so this is deliberately
/// not re-derived on every lookup).
pub struct GameRoot {
    root: PathBuf,
    catalog: AssetCatalog,
}

impl GameRoot {
    /// Builds a [`GameRoot`] by walking `root` exactly as the real
    /// game's virtual file system does. Does not require the root to
    /// actually be a valid PAL3 install — use
    /// [`GameRoot::looks_like_pal3`] to check that.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let (_vfs, catalog) = packfs::init_virtual_fs_with_catalog(&root, None);
        Self { root, catalog }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn catalog(&self) -> &AssetCatalog {
        &self.catalog
    }

    /// Whether this root's catalog contains PAL3's marker package
    /// (`basedata/basedata.cpk`, mounted at `/basedata/basedata`).
    pub fn looks_like_pal3(&self) -> bool {
        self.catalog.mounts().iter().any(|m| {
            m.package_type == PackageType::Cpk
                && m.vfs_mount_point == Path::new(PAL3_MARKER_VFS_MOUNT)
        })
    }

    /// Full physical path a `target_package` (as recorded in a
    /// `.ybpatch`'s `AssetChange::target_package`, e.g. `"scene.cpk"`
    /// or `"basedata/basedata.cpk"`, always forward-slash separated)
    /// resolves to under this root, if it's actually mounted.
    pub fn resolve_package_path(&self, target_package: &str) -> Option<PathBuf> {
        let wanted = normalize(target_package);
        self.catalog
            .mounts()
            .iter()
            .find(|m| normalize_path(&m.physical_relative_path) == wanted)
            .map(|m| self.catalog.physical_path(m))
    }
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn normalize_path(path: &Path) -> String {
    normalize(&path.to_string_lossy())
}

/// Enumerates plausible PAL3 install roots to probe, in priority
/// order: an explicit override (e.g. a CLI arg or remembered GUI
/// selection), the current working directory, and the platform config
/// file's remembered `asset_path` for `pal3` (see
/// [`crate::config::configured_pal3_asset_path`]).
pub fn candidate_roots(explicit: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p);
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Some(configured) = crate::config::configured_pal3_asset_path() {
        candidates.push(configured);
    }
    candidates
}

/// Probes `candidates` in order and returns the first that looks like
/// a real PAL3 install (see [`GameRoot::looks_like_pal3`]).
pub fn detect_pal3_root(candidates: impl IntoIterator<Item = PathBuf>) -> Option<GameRoot> {
    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        let root = GameRoot::open(&candidate);
        if root.looks_like_pal3() {
            return Some(root);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_directory_does_not_look_like_pal3() {
        let dir = crate::test_scratch::dir("environment-empty");

        let root = GameRoot::open(&dir);
        assert!(!root.looks_like_pal3());
        assert!(root.resolve_package_path("basedata/basedata.cpk").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_unrelated_cpk_does_not_hide_valid_pal3_root() {
        let dir = crate::test_scratch::dir("environment-malformed-unrelated-cpk");
        crate::fixtures::write_fixture_cpk(
            &dir.join("basedata"),
            "basedata.cpk",
            &[("marker.txt", b"basedata marker")],
        );
        std::fs::write(dir.join("truncated.cpk"), [0_u8; 16]).unwrap();

        let root = GameRoot::open(&dir);
        assert!(root.looks_like_pal3());
        assert!(
            root.resolve_package_path("truncated.cpk").is_none(),
            "unreadable packages must not be recorded as mounted"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
