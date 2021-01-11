use super::{cpk_archive::CpkFile, CpkArchive};
use encoding::{EncoderTrap, Encoding};
use log::debug;
use memmap::{Mmap, MmapOptions};
use mini_fs::Store;
use std::{
    cell::RefCell,
    fs::File,
    io::{Cursor, Result},
    path::Path,
};

pub struct CpkFs {
    cpk_archive: RefCell<CpkArchive<Mmap>>,
}

impl CpkFs {
    pub fn new<P: AsRef<Path>>(cpk_path: P) -> Result<CpkFs> {
        let file = File::open(cpk_path)?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let cpk_archive = RefCell::new(CpkArchive::load(cursor)?);

        Ok(CpkFs { cpk_archive })
    }
}

impl Store for CpkFs {
    type File = CpkFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        debug!("Opening {:?}", &path);
        self.cpk_archive.borrow_mut().open(
            &encoding::all::GBK
                .encode(&path.to_str().unwrap().to_lowercase(), EncoderTrap::Ignore)
                .unwrap(),
        )
    }
}
