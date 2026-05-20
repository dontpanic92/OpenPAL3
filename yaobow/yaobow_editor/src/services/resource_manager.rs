//! Category-based resource browser. Walks the editor's `MiniFs` once
//! and buckets every file by extension, then serves filtered queries
//! to the resources panel.
//!
//! Read-only; no mutation paths. The index is built lazily on the
//! first method call and cached for the hub's lifetime — the editor's
//! VFS is mounted once per opened game and never changes underneath
//! us, so a one-time scan is sufficient.

use std::cell::RefCell;
use std::ffi::OsStr;
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
                // Some MiniFs stores (notably `LocalFs`) return entry
                // names relative to the *store root* rather than to
                // the queried directory — e.g. listing "/scene/Q01"
                // yields entries named "scene/Q01/Q01.scn". Joining
                // those raw against `dir` doubles the prefix and
                // produces broken paths (`/scene/Q01/scene/Q01/...`).
                // Mirror `VfsService::entry_display_name` and keep
                // just the basename so the resulting paths line up
                // with what the rest of the editor (vfs + previewer
                // hub) actually opens.
                let name = display_name(&entry.name);
                if name.is_empty() {
                    continue;
                }
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

/// Reduce a raw `mini_fs::Entry::name` to a basename. Some store
/// implementations populate `name` with the path relative to the
/// store root rather than the queried directory; trimming to the
/// last path component keeps the resource browser aligned with the
/// vfs path scheme everywhere else in the editor.
fn display_name(raw: &OsStr) -> String {
    let path = PathBuf::from(raw);
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| raw.to_string_lossy().to_string())
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
    bucket.iter().filter(|p| matches(p, &f)).nth(row).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn display_name_strips_path_prefix() {
        // LocalFs returns entry names relative to the store root,
        // e.g. "scene/Q01/Q01.scn" when listing "/scene/Q01". The
        // helper must reduce that to just "Q01.scn".
        assert_eq!(
            display_name(&OsString::from("scene/Q01/Q01.scn")),
            "Q01.scn"
        );
        assert_eq!(display_name(&OsString::from("Q01.scn")), "Q01.scn");
        assert_eq!(display_name(&OsString::from("scene")), "scene");
    }

    #[test]
    fn category_index_emits_clean_paths_for_nested_localfs() {
        // Regression: walking a MiniFs(LocalFs) tree previously
        // produced doubled prefixes like
        // "/scene/Q01/scene/Q01/Q01.scn" because entry names from
        // LocalFs are relative to the store root, not the queried
        // directory. The browser ended up listing paths the rest
        // of the editor could not open.
        let root =
            std::env::temp_dir().join(format!("yaobow_editor_rm_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("scene/Q01")).unwrap();
        fs::write(root.join("scene/Q01/Q01.scn"), b"").unwrap();
        fs::write(root.join("root.txt"), b"").unwrap();

        let vfs = Rc::new(mini_fs::MiniFs::new(false).mount("/", mini_fs::LocalFs::new(&root)));
        let rm = ResourceManager::create(vfs);

        let scenes_idx = CATEGORIES.iter().position(|c| *c == "Scenes").unwrap() as i32;
        assert_eq!(rm.category_entry_count(scenes_idx, ""), 1);
        assert_eq!(
            rm.category_entry_path(scenes_idx, "", 0).to_string(),
            "/scene/Q01/Q01.scn"
        );

        let data_idx = CATEGORIES.iter().position(|c| *c == "Data").unwrap() as i32;
        assert_eq!(
            rm.category_entry_path(data_idx, "", 0).to_string(),
            "/root.txt"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
