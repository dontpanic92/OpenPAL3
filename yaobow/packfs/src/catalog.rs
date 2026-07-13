//! Provenance catalog for packages mounted by [`crate::init_virtual_fs`].
//!
//! [`crate::init_virtual_fs`] walks the local asset directory and mounts
//! every recognized package (`.cpk`, `.fmb`, `.imd`, `.sfb`, `.pkg`,
//! `.zpk`, `.zpkg`, `.zip`, `.ypk`) into a [`mini_fs::MiniFs`], but throws
//! away exactly the information a patcher needs afterwards: which
//! physical file backs a given VFS path, and what the path *inside* that
//! package would be. [`AssetCatalog`] (produced by
//! [`crate::init_virtual_fs_with_catalog`]) records that provenance
//! without changing `init_virtual_fs`'s existing behavior/signature.

use std::path::{Path, PathBuf};

/// Kind of package a [`MountedPackage`] refers to. Mirrors the file
/// extensions recognized by `mount_packages_recursive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageType {
    Cpk,
    Fmb,
    Imd,
    Sfb,
    Pkg,
    Zpk,
    Zpkg,
    Zip,
    Ypk,
}

impl PackageType {
    /// Maps a file extension (without the leading dot, any case) to a
    /// [`PackageType`], if recognized.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "cpk" => Some(PackageType::Cpk),
            "fmb" => Some(PackageType::Fmb),
            "imd" => Some(PackageType::Imd),
            "sfb" => Some(PackageType::Sfb),
            "pkg" => Some(PackageType::Pkg),
            "zpk" => Some(PackageType::Zpk),
            "zpkg" => Some(PackageType::Zpkg),
            "zip" => Some(PackageType::Zip),
            "ypk" => Some(PackageType::Ypk),
            _ => None,
        }
    }
}

/// A single package mounted into the virtual file system.
#[derive(Debug, Clone)]
pub struct MountedPackage {
    /// Path to the package file, relative to the local asset root passed
    /// to [`crate::init_virtual_fs_with_catalog`], using the host's
    /// native path separators (e.g. `scene/q01.cpk`).
    pub physical_relative_path: PathBuf,
    /// Absolute mount point inside the virtual file system this package
    /// was mounted at (e.g. `/scene/q01`).
    pub vfs_mount_point: PathBuf,
    /// Package format.
    pub package_type: PackageType,
}

/// Provenance record of every package mounted by
/// [`crate::init_virtual_fs_with_catalog`].
///
/// This is what lets a "patcher" go from a VFS path (as used by game
/// code / scripts) back to the physical package file — and the relative
/// path *inside* that package — that a format-specific rebuilder (e.g.
/// [`crate::cpk::CpkRebuilder`]) needs to operate on.
#[derive(Debug, Clone, Default)]
pub struct AssetCatalog {
    local_asset_root: PathBuf,
    mounts: Vec<MountedPackage>,
}

impl AssetCatalog {
    pub fn new<P: AsRef<Path>>(local_asset_root: P) -> Self {
        AssetCatalog {
            local_asset_root: local_asset_root.as_ref().to_path_buf(),
            mounts: vec![],
        }
    }

    /// The local asset root this catalog was built against.
    pub fn local_asset_root(&self) -> &Path {
        &self.local_asset_root
    }

    /// All packages mounted into the virtual file system, in mount
    /// order (matching `mini_fs::MiniFs`'s internal mount list order).
    pub fn mounts(&self) -> &[MountedPackage] {
        &self.mounts
    }

    pub(crate) fn record(
        &mut self,
        physical_relative_path: PathBuf,
        vfs_mount_point: PathBuf,
        package_type: PackageType,
    ) {
        self.mounts.push(MountedPackage {
            physical_relative_path,
            vfs_mount_point,
            package_type,
        });
    }

    /// Resolves an absolute VFS path (e.g. `/scene/q01/q01.scn`) to the
    /// package that would actually serve it, plus the path *inside* that
    /// package (relative to its mount point).
    ///
    /// Mirrors `mini_fs::MiniFs::open_path`'s own resolution order
    /// (mounts are searched most-recently-mounted-first, first prefix
    /// match wins) so `resolve()` always agrees with what a live
    /// `vfs.open(vfs_path)` would actually read from.
    pub fn resolve<'a>(&'a self, vfs_path: &Path) -> Option<(&'a MountedPackage, PathBuf)> {
        self.mounts.iter().rev().find_map(|mount| {
            strip_prefix_case_insensitive(vfs_path, &mount.vfs_mount_point)
                .map(|internal| (mount, internal))
        })
    }

    /// Full physical filesystem path to `mount`'s package file.
    pub fn physical_path(&self, mount: &MountedPackage) -> PathBuf {
        self.local_asset_root.join(&mount.physical_relative_path)
    }
}

fn strip_prefix_case_insensitive(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let path_components: Vec<_> = path.components().collect();
    let prefix_components: Vec<_> = prefix.components().collect();
    if prefix_components.len() > path_components.len() {
        return None;
    }
    if !path_components
        .iter()
        .zip(&prefix_components)
        .all(|(a, b)| {
            a.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
        })
    {
        return None;
    }
    Some(path_components[prefix_components.len()..].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_matches_mount_points_case_insensitively() {
        let mut catalog = AssetCatalog::new("/game");
        catalog.record(
            PathBuf::from("Scene/Q01.cpk"),
            PathBuf::from("/Scene/Q01"),
            PackageType::Cpk,
        );

        let (mount, internal) = catalog
            .resolve(Path::new("/scene/q01/Model/Role.mv3"))
            .expect("case-insensitive VFS should resolve");
        assert_eq!(mount.physical_relative_path, PathBuf::from("Scene/Q01.cpk"));
        assert_eq!(internal, PathBuf::from("Model/Role.mv3"));
    }
}
