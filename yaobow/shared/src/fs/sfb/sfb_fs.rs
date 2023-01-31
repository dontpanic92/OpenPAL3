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

use super::sfb_archive::SfbArchive;

pub struct SfbFs {
    sfb_archive: RefCell<SfbArchive<Mmap>>,
}

impl SfbFs {
    pub fn new<P: AsRef<Path>>(sfb_path: P) -> anyhow::Result<SfbFs> {
        let file = File::open(sfb_path.as_ref())?;
        let mem = unsafe { MmapOptions::new().map(&file)? };
        let cursor = Cursor::new(mem);
        let sfb_archive = RefCell::new(SfbArchive::load(cursor)?);
        Ok(SfbFs { sfb_archive })
    }
}

impl Store for SfbFs {
    type File = MemoryFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.sfb_archive
            .borrow_mut()
            .open(path)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    fn entries_path(&self, _: &Path) -> io::Result<Entries> {
        let archive = self.sfb_archive.borrow();
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
