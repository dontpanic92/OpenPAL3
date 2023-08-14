use crate::fs::{create_reader, memory_file::MemoryFile};
use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{
    cell::RefCell,
    ffi::OsString,
    io::{self},
    path::Path,
};

use super::pkg_archive::{PkgArchive, PkgEntry};

pub struct PkgFs {
    pkg_archive: RefCell<PkgArchive>,
}

impl PkgFs {
    pub fn new<P: AsRef<Path>>(pkg_path: P, decrypt_key: &str) -> anyhow::Result<PkgFs> {
        let reader = create_reader(pkg_path)?;
        let pkg_archive = RefCell::new(PkgArchive::load(reader, decrypt_key)?);
        Ok(PkgFs { pkg_archive })
    }
}

impl Store for PkgFs {
    type File = MemoryFile;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        let path = path.to_string_lossy().replace("/", r"\");
        let path = Path::new(path.chars().as_str());
        self.pkg_archive
            .borrow_mut()
            .open(path)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    fn entries_path(&self, p: &Path) -> io::Result<Entries> {
        let archive = self.pkg_archive.borrow();
        let list = archive.entries.root_entry.list_content(p)?;
        let entries = list.into_iter().map(|e| {
            Ok(Entry {
                name: OsString::from(e.name()),
                kind: if let PkgEntry::Folder(_) = e {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                },
            })
        });

        Ok(Entries::new(entries))
    }
}
