use std::io::{BufReader, Read};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};

use crate::comdef::services::{IVfsService, IVfsServiceImpl};

pub struct VfsService {
    vfs: Rc<MiniFs>,
}

ComObject_VfsService!(super::VfsService);

impl VfsService {
    pub fn create(vfs: Rc<MiniFs>) -> ComRc<IVfsService> {
        ComRc::from_object(Self { vfs })
    }
}

impl IVfsServiceImpl for VfsService {
    fn exists(&self, vfs_path: &str) -> bool {
        self.vfs.open(vfs_path).is_ok()
    }

    fn read_bytes_internal(&self, vfs_path: &str) -> Vec<u8> {
        let Ok(file) = self.vfs.open(vfs_path) else {
            return Vec::new();
        };
        let mut bytes = Vec::new();
        if BufReader::new(file).read_to_end(&mut bytes).is_ok() {
            bytes
        } else {
            Vec::new()
        }
    }

    fn byte_len(&self, vfs_path: &str) -> i32 {
        if !self.exists(vfs_path) {
            return -1;
        }
        self.read_bytes_internal(vfs_path)
            .len()
            .try_into()
            .unwrap_or(i32::MAX)
    }
}
