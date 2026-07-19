use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::OsString,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use mini_fs::{Entries, Entry, EntryKind, Store};

use crate::YpkArchive;

pub struct YpkFs {
    archive: RefCell<YpkArchive>,
}

impl YpkFs {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let archive = RefCell::new(YpkArchive::load(Arc::new(Mutex::new(file)))?);
        Ok(Self { archive })
    }

    pub fn from_bytes(bytes: &'static [u8]) -> anyhow::Result<Self> {
        let archive = RefCell::new(YpkArchive::load(Arc::new(Mutex::new(Cursor::new(bytes))))?);
        Ok(Self { archive })
    }
}

impl Store for YpkFs {
    type File = mini_fs::File;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.archive.borrow_mut().open(path.to_str().unwrap())
    }

    fn entries_path(&self, path: &Path) -> std::io::Result<Entries<'_>> {
        let directory = path
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches(|character| character == '/' || character == '.')
            .trim_end_matches('/')
            .to_string();
        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };

        let archive = self.archive.borrow();
        let mut children = BTreeMap::new();
        for entry in &archive.entries {
            let Some(relative) = entry.name().strip_prefix(&prefix) else {
                continue;
            };
            if relative.is_empty() {
                continue;
            }
            if let Some(slash) = relative.find('/') {
                children
                    .entry(relative[..slash].to_string())
                    .or_insert(EntryKind::Dir);
            } else {
                children.insert(relative.to_string(), EntryKind::File);
            }
        }

        let entries: Vec<std::io::Result<Entry>> = children
            .into_iter()
            .map(|(name, kind)| {
                Ok(Entry {
                    name: OsString::from(name),
                    kind,
                })
            })
            .collect();
        Ok(Entries::new(entries.into_iter()))
    }
}
