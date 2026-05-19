//! Category-based resource browser. Walks the editor's `MiniFs` once
//! and buckets every file by extension, then serves filtered queries
//! to the resources panel.
//!
//! Read-only; no mutation paths. The index is built lazily on the
//! first method call and cached for the hub's lifetime — the editor's
//! VFS is mounted once per opened game and never changes underneath
//! us, so a one-time scan is sufficient.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{EntryKind, MiniFs, StoreExt};

use crate::comdef::editor_services::{IResourceManager, IResourceManagerImpl};

const CATEGORIES: &[&str] = &[
    "Scenes",   // 0
    "Models",   // 1
    "Textures", // 2
    "Audio",    // 3
    "Video",    // 4
    "Scripts",  // 5
    "Data",     // 6
    "Other",    // 7
];

fn classify(ext: &str) -> usize {
    match ext {
        "scn" | "nav" | "bsp" | "gob" => 0,
        "mv3" | "cvd" | "dff" | "anm" | "pol" => 1,
        "tga" | "png" | "dds" | "bmp" | "jpg" | "jpeg" => 2,
        "mp3" | "smp" | "wav" | "ogg" => 3,
        "bik" => 4,
        "sce" | "nod" | "h" | "asm" => 5,
        "ini" | "txt" | "conf" | "cfg" | "log" | "csv" | "xml" | "json" => 6,
        _ => 7,
    }
}

pub struct ResourceManager {
    vfs: Rc<MiniFs>,
    buckets: RefCell<Option<Vec<Vec<String>>>>,
    last_string: RefCell<String>,
}

ComObject_ResourceManager!(super::ResourceManager);

impl ResourceManager {
    pub fn create(vfs: Rc<MiniFs>) -> ComRc<IResourceManager> {
        ComRc::from_object(Self {
            vfs,
            buckets: RefCell::new(None),
            last_string: RefCell::new(String::new()),
        })
    }

    fn ensure_built(&self) {
        if self.buckets.borrow().is_some() {
            return;
        }
        let mut buckets: Vec<Vec<String>> = (0..CATEGORIES.len()).map(|_| Vec::new()).collect();
        // Depth-first walk over MiniFs.
        let mut stack: Vec<PathBuf> = vec![PathBuf::from("/")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = self.vfs.entries(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.name.to_string_lossy().to_string();
                let child = join_path(&dir, &name);
                match entry.kind {
                    EntryKind::Dir => stack.push(child),
                    EntryKind::File => {
                        let ext = Path::new(&name)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default();
                        let bucket = classify(&ext);
                        if let Some(s) = child.to_str() {
                            buckets[bucket].push(s.to_string());
                        }
                    }
                }
            }
        }
        for b in &mut buckets {
            b.sort();
        }
        *self.buckets.borrow_mut() = Some(buckets);
    }

    fn set_last(&self, s: String) -> &str {
        *self.last_string.borrow_mut() = s;
        // SAFETY: see ConfigService::get_asset_path — single-threaded
        // script/UI path; codegen copies the &str into a CString
        // immediately on return.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

fn join_path(dir: &Path, name: &str) -> PathBuf {
    let d = dir.to_string_lossy();
    if d == "/" || d.is_empty() {
        PathBuf::from(format!("/{}", name))
    } else {
        PathBuf::from(format!("{}/{}", d.trim_end_matches('/'), name))
    }
}

impl IResourceManagerImpl for ResourceManager {
    fn category_count(&self) -> i32 {
        CATEGORIES.len() as i32
    }

    fn category_name(&self, idx: i32) -> &str {
        CATEGORIES.get(idx as usize).copied().unwrap_or("")
    }

    fn category_entry_count(&self, idx: i32, filter: &str) -> i32 {
        self.ensure_built();
        let buckets = self.buckets.borrow();
        let Some(buckets) = buckets.as_ref() else {
            return 0;
        };
        let Some(bucket) = buckets.get(idx as usize) else {
            return 0;
        };
        count_matches(bucket, filter) as i32
    }

    fn category_entry_path(&self, idx: i32, filter: &str, row: i32) -> &str {
        self.ensure_built();
        let path = {
            let buckets = self.buckets.borrow();
            let Some(buckets) = buckets.as_ref() else {
                return self.set_last(String::new());
            };
            let Some(bucket) = buckets.get(idx as usize) else {
                return self.set_last(String::new());
            };
            nth_match(bucket, filter, row.max(0) as usize).unwrap_or_default()
        };
        self.set_last(path)
    }
}

fn matches(path: &str, filter_lower: &str) -> bool {
    filter_lower.is_empty() || path.to_lowercase().contains(filter_lower)
}

fn count_matches(bucket: &[String], filter: &str) -> usize {
    let f = filter.to_lowercase();
    bucket.iter().filter(|p| matches(p, &f)).count()
}

fn nth_match(bucket: &[String], filter: &str, row: usize) -> Option<String> {
    let f = filter.to_lowercase();
    bucket
        .iter()
        .filter(|p| matches(p, &f))
        .nth(row)
        .cloned()
}