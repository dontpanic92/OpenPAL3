#![cfg_attr(target_os = "vita", feature(stdarch_arm_neon_intrinsics))]

pub mod catalog;
pub mod cpk;
pub mod fmb;
pub mod imd;
pub mod memory_file;
pub mod pkg;
pub mod plain_fs;
pub mod sfb;
pub mod streaming_file;
pub mod ypk;
pub mod zpk;
pub mod zpkg;

use std::{
    fs,
    path::{Path, PathBuf},
};

use common::SeekRead;
use mini_fs::{LocalFs, MiniFs, ZipFs};

pub use catalog::{AssetCatalog, MountedPackage, PackageType};

use crate::{
    cpk::CpkFs, fmb::fmb_fs::FmbFs, imd::imd_fs::ImdFs, pkg::pkg_fs::PkgFs, sfb::sfb_fs::SfbFs,
    zpk::zpk_fs::ZpkFs, zpkg::zpkg_fs::ZpkgFs,
};

pub fn init_virtual_fs<P: AsRef<Path>>(local_asset_path: P, pkg_key: Option<&str>) -> MiniFs {
    init_virtual_fs_with_catalog(local_asset_path, pkg_key).0
}

/// Same as [`init_virtual_fs`], but additionally returns an
/// [`AssetCatalog`] recording the physical relative path, VFS mount
/// point, and format of every package that got mounted. Useful for a
/// patcher that needs to map a VFS path back to the on-disk package
/// (and internal path within it) that should be rebuilt — see
/// [`AssetCatalog::resolve`] and [`cpk::CpkRebuilder`].
pub fn init_virtual_fs_with_catalog<P: AsRef<Path>>(
    local_asset_path: P,
    pkg_key: Option<&str>,
) -> (MiniFs, AssetCatalog) {
    log::debug!(
        "Initializing virtual file system with {:?}",
        local_asset_path.as_ref()
    );
    let local = LocalFs::new(local_asset_path.as_ref());
    let vfs = MiniFs::new(false).mount("/", local);
    let mut catalog = AssetCatalog::new(local_asset_path.as_ref());
    let vfs = mount_packages_recursive(
        vfs,
        local_asset_path.as_ref(),
        &PathBuf::from("./"),
        pkg_key,
        &mut catalog,
    );

    (vfs, catalog)
}

fn mount_packages_recursive(
    mut vfs: MiniFs,
    local_path: &Path,
    relative_path: &Path,
    pkg_key: Option<&str>,
    catalog: &mut AssetCatalog,
) -> MiniFs {
    let path = local_path.join(relative_path);
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().eq_ignore_ascii_case("PALSound") {
                continue;
            }

            let new_path = relative_path.join(entry.file_name());
            vfs = mount_packages_recursive(vfs, local_path, &new_path, pkg_key.clone(), catalog);
        }
    } else {
        let vfs_path = PathBuf::from("/").join(relative_path.with_extension(""));
        // Physical path recorded in the catalog, relative to
        // `local_path`, without the `mount_packages_recursive` internal
        // `./` recursion prefix.
        let physical_relative_path = relative_path
            .strip_prefix("./")
            .unwrap_or(relative_path)
            .to_path_buf();

        match path.extension().and_then(|ext| ext.to_str()) {
            Some("cpk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                match CpkFs::new(&path) {
                    Ok(cpk) => {
                        vfs = vfs.mount(vfs_path.clone(), cpk);
                        catalog.record(physical_relative_path, vfs_path, PackageType::Cpk);
                    }
                    Err(error) => {
                        log::error!("Skipping unreadable CPK package {:?}: {error:#}", &path);
                    }
                }
            }
            Some("fmb") => {
                let vfs_path = vfs_path.parent().unwrap().join("Model");
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), FmbFs::create(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Fmb);
            }
            Some("imd") => {
                let vfs_path = vfs_path.parent().unwrap().join("Texture");
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), ImdFs::create(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Imd);
            }
            Some("sfb") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), SfbFs::create(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Sfb);
            }
            Some("pkg") => match pkg_key {
                None => log::debug!("Didn't mount {:?} as pkg key is not provided", &path),
                Some(key) => {
                    log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                    vfs = vfs.mount(vfs_path.clone(), PkgFs::new(path, key).unwrap());
                    catalog.record(physical_relative_path, vfs_path, PackageType::Pkg);
                }
            },
            Some("zpk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), ZpkFs::create(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Zpk);
            }
            Some("zpkg") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), ZpkgFs::create(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Zpkg);
            }
            Some("zip") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                let z = ZipFs::open(path).unwrap();
                vfs = vfs.mount(vfs_path.clone(), z);
                catalog.record(physical_relative_path, vfs_path, PackageType::Zip);
            }
            Some("ypk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path.clone(), ypk::YpkFs::new(path).unwrap());
                catalog.record(physical_relative_path, vfs_path, PackageType::Ypk);
            }
            _ => {}
        }
    }

    vfs
}

#[cfg(vita)]
fn create_reader<P: AsRef<Path>>(path: P) -> anyhow::Result<Box<dyn SeekRead>> {
    let file = std::fs::File::open(path.as_ref())?;
    Ok(Box::new(file))
}

#[cfg(any(windows, linux, macos, android))]
fn create_reader<P: AsRef<Path>>(path: P) -> anyhow::Result<Box<dyn SeekRead>> {
    let file = std::fs::File::open(path.as_ref())?;
    let mem = unsafe { memmap::MmapOptions::new().map(&file)? };
    Ok(Box::new(std::io::Cursor::new(mem)))
}
