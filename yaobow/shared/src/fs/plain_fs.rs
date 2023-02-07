use crate::fs::memory_file::MemoryFile;
use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{cell::RefCell, ffi::OsString, path::Path};

pub trait PlainArchive {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile>;
    fn files(&self) -> Vec<String>;
}

pub struct PlainFs<TArchive: PlainArchive> {
    archive: RefCell<TArchive>,
}

impl<TArchive: PlainArchive> PlainFs<TArchive> {
    pub fn new(archive: TArchive) -> Self {
        Self {
            archive: RefCell::new(archive),
        }
    }
}

impl<TArchive: PlainArchive> Store for PlainFs<TArchive> {
    type File = MemoryFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.archive
            .borrow_mut()
            .open(path)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    fn entries_path(&self, _: &Path) -> std::io::Result<Entries> {
        let archive = self.archive.borrow();
        let list: Vec<Result<Entry, std::io::Error>> = archive
            .files()
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
