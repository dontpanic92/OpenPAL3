use std::path::Path;

use crate::fs::{create_reader, plain_fs::PlainFs};

use super::zpk_archive::ZpkArchive;

pub type ZpkFs = PlainFs<ZpkArchive>;

impl ZpkFs {
    pub fn create<P: AsRef<Path>>(zpk_path: P) -> anyhow::Result<ZpkFs> {
        let reader = create_reader(zpk_path)?;
        let sfb_archive = ZpkArchive::load(reader)?;
        Ok(Self::new(sfb_archive))
    }
}
