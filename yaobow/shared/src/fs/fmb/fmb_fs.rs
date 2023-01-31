use crate::fs::memory_file::MemoryFile;
use memmap::{Mmap, MmapOptions};
use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{
    cell::RefCell,
    ffi::OsString,
    fs::File,
    io::{self, Cursor},
    path::Path,
};

use super::fmb_archive::FmbArchive;

pub struct FmbFs {
    fmb_archive: RefCell<FmbArchive<Mmap>>,
}

impl FmbFs {
    pub fn new<P: AsRef<Path>>(fmb_path: P) -> anyhow::Result<FmbFs> {
        let file = File::open(fmb_path.as_ref())?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let fmb_archive = RefCell::new(FmbArchive::load(cursor)?);
        Ok(FmbFs { fmb_archive })
    }
}

impl Store for FmbFs {
    type File = MemoryFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.fmb_archive
            .borrow_mut()
            .open(path)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    fn entries_path(&self, _: &Path) -> io::Result<Entries> {
        let archive = self.fmb_archive.borrow();
        let list: Vec<Result<Entry, io::Error>> = archive
            .files
            .keys()
            .into_iter()
            .map(|name| {
                Ok(Entry {
                    name: OsString::from(name),
                    kind: EntryKind::File,
                })
            })
            .collect();

        Ok(Entries::new(list))
    }
}
