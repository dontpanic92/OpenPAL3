pub mod cpk;
pub mod memory_file;
pub mod pkg;

use std::{
    fs,
    path::{Path, PathBuf},
};

use mini_fs::{LocalFs, MiniFs};

use crate::fs::cpk::CpkFs;

pub fn init_virtual_fs<P: AsRef<Path>>(local_asset_path: P) -> MiniFs {
    let local = LocalFs::new(local_asset_path.as_ref());
    let vfs = MiniFs::new(false).mount("/", local);
    let vfs = mount_pack_recursive(vfs, local_asset_path.as_ref(), &PathBuf::from("./"));

    vfs
}

fn mount_pack_recursive(mut vfs: MiniFs, local_path: &Path, relative_path: &Path) -> MiniFs {
    let path = local_path.join(relative_path);
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let new_path = relative_path.join(entry.file_name());
            vfs = mount_pack_recursive(vfs, local_path, &new_path);
        }
    } else {
        let vfs_path = PathBuf::from("/").join(relative_path.with_extension(""));
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("cpk") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, CpkFs::new(path).unwrap())
            }
            Some("pkg") => {
                log::debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, CpkFs::new(path).unwrap())
            }
            _ => {}
        }
    }

    vfs
}
