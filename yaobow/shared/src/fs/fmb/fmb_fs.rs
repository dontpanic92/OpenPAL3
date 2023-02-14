use crate::fs::plain_fs::PlainFs;
use memmap::{Mmap, MmapOptions};
use std::{fs::File, io::Cursor, path::Path};

use super::fmb_archive::FmbArchive;

pub type FmbFs = PlainFs<FmbArchive<Mmap>>;

impl FmbFs {
    pub fn create<P: AsRef<Path>>(fmb_path: P) -> anyhow::Result<Self> {
        let file = File::open(fmb_path.as_ref())?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let sfb_archive = FmbArchive::load(cursor)?;
        Ok(Self::new(sfb_archive))
    }
}
