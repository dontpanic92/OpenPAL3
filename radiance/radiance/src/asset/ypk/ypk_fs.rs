use mini_fs::{Entries, Entry, EntryKind, Store};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::OsString,
    path::Path,
    sync::{Arc, Mutex},
};

use super::YpkArchive;

pub struct YpkFs {
    archive: RefCell<YpkArchive>,
}

impl YpkFs {
    pub fn new<P: AsRef<Path>>(ypk_path: P) -> anyhow::Result<YpkFs> {
        let file = std::fs::File::open(ypk_path.as_ref())?;
        let archive = RefCell::new(YpkArchive::load(Arc::new(Mutex::new(file)))?);
        Ok(YpkFs { archive })
    }
}

impl Store for YpkFs {
    type File = mini_fs::File;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.archive.borrow_mut().open(path.to_str().unwrap())
    }

    fn entries_path(&self, path: &Path) -> std::io::Result<Entries<'_>> {
        // Normalise the requested directory to the same shape entries
        // are stored in: forward slashes, no leading `/` or `.`, no
        // trailing `/`. The root directory is the empty string.
        let dir = path
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches(|c| c == '/' || c == '.')
            .trim_end_matches('/')
            .to_string();
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };

        // Walk every entry once, collecting only the immediate children
        // under `prefix`. Directories are inferred from any entry that
        // has at least one more `/` after the prefix. Dedup via the
        // BTreeMap key so each child appears exactly once. O(N) per
        // listing — acceptable because directory listings on YPK
        // bundles are an editor/debug path, not a hot read path.
        let archive = self.archive.borrow();
        let mut children: BTreeMap<String, EntryKind> = BTreeMap::new();
        for entry in &archive.entries {
            let name = entry.name();
            let Some(rel) = name.strip_prefix(&prefix) else {
                continue;
            };
            if rel.is_empty() {
                continue;
            }
            match rel.find('/') {
                None => {
                    children.insert(rel.to_string(), EntryKind::File);
                }
                Some(slash) => {
                    let child = &rel[..slash];
                    // File wins over a previous Dir marker only if it
                    // already exists; we always prefer Dir when the
                    // child has descendants. `or_insert` handles both
                    // cases (the first observation sticks for that
                    // child name; subsequent loops for the same dir
                    // are no-ops).
                    children
                        .entry(child.to_string())
                        .or_insert(EntryKind::Dir);
                }
            }
        }

        let collected: Vec<std::io::Result<Entry>> = children
            .into_iter()
            .map(|(name, kind)| {
                Ok(Entry {
                    name: OsString::from(name),
                    kind,
                })
            })
            .collect();
        Ok(Entries::new(collected.into_iter()))
    }
}

