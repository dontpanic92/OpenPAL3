use crate::fs::{create_reader, memory_file::MemoryFile};

use super::{CpkArchive, CpkEntry};
use encoding::{EncoderTrap, Encoding};
use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{cell::RefCell, ffi::OsString, path::Path, rc::Rc};

pub struct CpkFs {
    cpk_archive: RefCell<CpkArchive>,
    entry: CpkEntry,
}

impl CpkFs {
    pub fn new<P: AsRef<Path>>(cpk_path: P) -> anyhow::Result<CpkFs> {
        let reader = create_reader(cpk_path)?;
        let cpk_archive = RefCell::new(CpkArchive::load(reader)?);
        let entry = cpk_archive.borrow().build_directory();

        Ok(CpkFs { cpk_archive, entry })
    }
}

impl Store for CpkFs {
    type File = MemoryFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        // need ad-hoc conversion to windows path
        // since the crc hashed path was hard-coded with back-slash dir separator
        let path = path.to_string_lossy().replace("/", r"\");
        let path = Path::new(path.chars().as_str());
        self.cpk_archive.borrow_mut().open(
            &encoding::all::GBK
                .encode(&path.to_str().unwrap().to_lowercase(), EncoderTrap::Ignore)
                .unwrap(),
        )
    }

    fn entries_path(&self, p: &Path) -> std::io::Result<Entries> {
        let entries = self.entry.ls(p)?;
        Ok(Entries::new(CpkEntryIter::new(Box::new(
            entries.into_iter(),
        ))))
    }
}

pub struct CpkEntryIter<'a> {
    entries: Box<dyn Iterator<Item = Rc<RefCell<CpkEntry>>> + 'a>,
}

impl<'a> CpkEntryIter<'a> {
    pub fn new(entries: Box<dyn Iterator<Item = Rc<RefCell<CpkEntry>>> + 'a>) -> Self {
        Self { entries }
    }
}

impl<'a> Iterator for CpkEntryIter<'a> {
    type Item = std::io::Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().and_then(|e| {
            Some(Ok(Entry {
                name: OsString::from(e.borrow().name()),
                kind: if e.borrow().is_dir() {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                },
            }))
        })
    }
}
