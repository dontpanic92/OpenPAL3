use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{cell::RefCell, ffi::OsString, path::Path};

use crate::memory_file::MemoryFile;

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
        let path = path.to_string_lossy().to_string().replace('$', "/");
        self.archive
            .borrow_mut()
            .open(path)
            .map_err(archive_error_to_io)
    }

    fn entries_path(&self, _: &Path) -> std::io::Result<Entries<'_>> {
        let archive = self.archive.borrow();
        let list: Vec<Result<Entry, std::io::Error>> = archive
            .files()
            .into_iter()
            .map(|name| {
                Ok(Entry {
                    name: OsString::from(name.replace('/', "$")),
                    kind: EntryKind::File,
                })
            })
            .collect();

        Ok(Entries::new(list))
    }
}

/// Flatten a [`PlainArchive`] error into an [`std::io::Error`] while
/// **preserving its [`std::io::ErrorKind`]**.
///
/// Every `PlainArchive` implementation reports a missing entry as
/// `std::io::Error::from(ErrorKind::NotFound)` wrapped in `anyhow`.
/// This used to be collapsed into `ErrorKind::Unsupported`, which broke
/// `MiniFs::open_path`: it only walks on to the next (lower-priority)
/// mount when a store answers `NotFound` and returns immediately on any
/// other error. Games that mount several archives at the same VFS point
/// — e.g. SWDHC's four `Texture_*.imd` files, all mounted at
/// `/Texture/Texture` — therefore lost every lookup that missed the
/// highest-priority archive, and their textures failed to resolve.
fn archive_error_to_io(error: anyhow::Error) -> std::io::Error {
    match error.downcast::<std::io::Error>() {
        Ok(io) => io,
        Err(other) => std::io::Error::new(std::io::ErrorKind::Other, other.to_string()),
    }
}
