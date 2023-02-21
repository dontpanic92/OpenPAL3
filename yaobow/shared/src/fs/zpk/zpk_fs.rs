use std::{fs::File, io::Cursor, path::Path};

use memmap::{Mmap, MmapOptions};

use crate::fs::plain_fs::PlainFs;

use super::zpk_archive::ZpkArchive;

pub type ZpkFs = PlainFs<ZpkArchive<Mmap>>;

impl ZpkFs {
    pub fn create<P: AsRef<Path>>(zpk_path: P) -> anyhow::Result<ZpkFs> {
        let file = File::open(zpk_path.as_ref())?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let sfb_archive = ZpkArchive::load(cursor)?;
        Ok(Self::new(sfb_archive))
    }
}
