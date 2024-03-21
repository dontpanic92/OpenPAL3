use crate::fs::{create_reader, plain_fs::PlainFs};
use std::path::Path;

use super::imd_archive::ImdArchive;

pub type ImdFs = PlainFs<ImdArchive>;

impl ImdFs {
    pub fn create<P: AsRef<Path>>(imd_path: P) -> anyhow::Result<Self> {
        let reader = create_reader(imd_path)?;
        let imd_archive = ImdArchive::load(reader)?;
        Ok(Self::new(imd_archive))
    }
}
