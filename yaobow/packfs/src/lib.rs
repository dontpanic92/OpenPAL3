#![feature(io_error_more)]
#![feature(cursor_remaining)]
#![cfg_attr(target_os = "vita", stdarch_arm_neon_intrinsics)]

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

use crate::{
    cpk::CpkFs, fmb::fmb_fs::FmbFs, imd::imd_fs::ImdFs, pkg::pkg_fs::PkgFs, sfb::sfb_fs::SfbFs,
    zpk::zpk_fs::ZpkFs, zpkg::zpkg_fs::ZpkgFs,
};

pub fn init_virtual_fs<P: AsRef<Path>>(local_asset_path: P, pkg_key: Option<&str>) -> MiniFs {
    let local = LocalFs::new(local_asset_path.as_ref());
    let vfs = MiniFs::new(false).mount("/", local);
    let vfs = mount_packages_recursive(
        vfs,
        local_asset_path.as_ref(),
        &PathBuf::from("./"),
        pkg_key,
    );

    vfs
}

fn mount_packages_recursive(
    mut vfs: MiniFs,
    local_path: &Path,
    relative_path: &Path,
    pkg_key: Option<&str>,
) -> MiniFs {
    let path = local_path.join(relative_path);
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().eq_ignore_ascii_case("PALSound") {
                continue;
            }

            let new_path = relative_path.join(entry.file_name());
            vfs = mount_packages_recursive(vfs, local_path, &new_path, pkg_key.clone());
        }
    } else {
        let vfs_path = PathBuf::from("/").join(relative_path.with_extension(""));
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("cpk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, CpkFs::new(path).unwrap())
            }
            Some("fmb") => {
                let vfs_path = vfs_path.parent().unwrap().join("Model");
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, FmbFs::create(path).unwrap())
            }
            Some("imd") => {
                let vfs_path = vfs_path.parent().unwrap().join("Texture");
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, ImdFs::create(path).unwrap())
            }
            Some("sfb") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, SfbFs::create(path).unwrap())
            }
            Some("pkg") => match pkg_key {
                None => log::debug!("Didn't mount {:?} as pkg key is not provided", &path),
                Some(key) => {
                    log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                    vfs = vfs.mount(vfs_path, PkgFs::new(path, key).unwrap())
                }
            },
            Some("zpk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, ZpkFs::create(path).unwrap())
            }
            Some("zpkg") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, ZpkgFs::create(path).unwrap())
            }
            Some("zip") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                let z = ZipFs::open(path).unwrap();
                vfs = vfs.mount(vfs_path, z);
            }
            Some("ypk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, ypk::YpkFs::new(path).unwrap())
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
