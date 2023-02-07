use crate::fs::plain_fs::PlainFs;
use memmap::{Mmap, MmapOptions};
use std::{fs::File, io::Cursor, path::Path};

use super::imd_archive::ImdArchive;

pub type ImdFs = PlainFs<ImdArchive<Mmap>>;

impl ImdFs {
    pub fn create<P: AsRef<Path>>(sfb_path: P) -> anyhow::Result<Self> {
        let file = File::open(sfb_path.as_ref())?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let sfb_archive = ImdArchive::load(cursor)?;
        Ok(Self::new(sfb_archive))
    }
}
