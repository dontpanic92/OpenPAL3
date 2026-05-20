use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{Entry, EntryKind, MiniFs, StoreExt};
use radiance::perf;

use crate::comdef::services::{IVfsService, IVfsServiceImpl};

pub struct VfsService {
    vfs: Rc<MiniFs>,
    entries_cache: RefCell<HashMap<String, Rc<Vec<Entry>>>>,
    path_is_dir: RefCell<HashMap<String, bool>>,
    command_paths: RefCell<Vec<String>>,
    path_commands: RefCell<HashMap<String, i32>>,
    expanded_paths: RefCell<HashSet<String>>,
    last_string: RefCell<String>,
    next_command_id: Cell<i32>,
}

ComObject_VfsService!(super::VfsService);

impl VfsService {
    pub fn create(vfs: Rc<MiniFs>) -> ComRc<IVfsService> {
        ComRc::from_object(Self {
            vfs,
            entries_cache: RefCell::new(HashMap::new()),
            path_is_dir: RefCell::new(HashMap::new()),
            command_paths: RefCell::new(Vec::new()),
            path_commands: RefCell::new(HashMap::new()),
            expanded_paths: RefCell::new(HashSet::new()),
            last_string: RefCell::new(String::new()),
            next_command_id: Cell::new(1),
        })
    }

    fn sorted_entries(&self, vfs_path: &str) -> Rc<Vec<Entry>> {
        if let Some(entries) = self.entries_cache.borrow().get(vfs_path) {
            return entries.clone();
        }

        perf::count("editor.tree.vfs.cache_misses", 1);
        let Ok(entries) = self.vfs.entries(Path::new(vfs_path)) else {
            let entries = Rc::new(Vec::new());
            self.entries_cache
                .borrow_mut()
                .insert(vfs_path.to_string(), entries.clone());
            return entries;
        };
        let mut entries: Vec<Entry> = entries.filter_map(Result::ok).collect();
        entries.sort_by(|a, b| match (a.kind, b.kind) {
            (EntryKind::Dir, EntryKind::Dir) => a.name.cmp(&b.name),
            (EntryKind::File, EntryKind::File) => a.name.cmp(&b.name),
            (EntryKind::Dir, EntryKind::File) => Ordering::Less,
            (EntryKind::File, EntryKind::Dir) => Ordering::Greater,
        });

        {
            let mut path_is_dir = self.path_is_dir.borrow_mut();
            path_is_dir.insert(vfs_path.to_string(), true);
            for entry in &entries {
                let child_path = join_vfs_path(vfs_path, &Self::entry_display_name(entry));
                path_is_dir.insert(child_path, entry.kind == EntryKind::Dir);
            }
        }
        let entries = Rc::new(entries);
        self.entries_cache
            .borrow_mut()
            .insert(vfs_path.to_string(), entries.clone());
        entries
    }

    fn entry_at(&self, vfs_path: &str, index: i32) -> Option<Entry> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.sorted_entries(vfs_path).get(index).cloned())
    }

    fn entry_display_name(entry: &Entry) -> String {
        PathBuf::from(&entry.name)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.name.to_string_lossy().to_string())
    }

    fn set_last_string(&self, value: String) -> &str {
        *self.last_string.borrow_mut() = value;
        // SAFETY: codegen copies the returned &str into a CString immediately.
        // The service is used on the single-threaded script/UI path, so no
        // nested mutable borrow can alter this buffer before that copy.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

fn join_vfs_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

impl VfsService {
    /// Read the full contents of `vfs_path` into a buffer. Engine-internal:
    /// the IDL surface intentionally does not expose this because every
    /// caller is Rust; scripts read via streaming-friendly methods like
    /// `byte_len` plus future range readers.
    fn read_bytes_internal(&self, vfs_path: &str) -> Vec<u8> {
        let Ok(file) = self.vfs.open(vfs_path) else {
            return Vec::new();
        };
        let mut bytes = Vec::new();
        if BufReader::new(file).read_to_end(&mut bytes).is_ok() {
            bytes
        } else {
            Vec::new()
        }
    }
}

impl IVfsServiceImpl for VfsService {
    fn exists(&self, vfs_path: &str) -> bool {
        self.vfs.open(vfs_path).is_ok()
    }

    fn byte_len(&self, vfs_path: &str) -> i32 {
        if !self.exists(vfs_path) {
            return -1;
        }
        self.read_bytes_internal(vfs_path)
            .len()
            .try_into()
            .unwrap_or(i32::MAX)
    }

    fn entry_count(&self, vfs_path: &str) -> i32 {
        perf::count("editor.tree.vfs.entry_count_calls", 1);
        self.sorted_entries(vfs_path)
            .len()
            .try_into()
            .unwrap_or(i32::MAX)
    }

    fn subdir_count(&self, vfs_path: &str) -> i32 {
        perf::count("editor.tree.vfs.subdir_count_calls", 1);
        let entries = self.sorted_entries(vfs_path);
        // sorted_entries places EntryKind::Dir first, so a linear
        // count from the front matches the directory prefix length.
        let n = entries
            .iter()
            .take_while(|e| e.kind == EntryKind::Dir)
            .count();
        n.try_into().unwrap_or(i32::MAX)
    }

    fn entry_name(&self, vfs_path: &str, index: i32) -> &str {
        perf::count("editor.tree.vfs.entry_name_calls", 1);
        let value = self
            .entry_at(vfs_path, index)
            .map(|entry| Self::entry_display_name(&entry))
            .unwrap_or_default();
        self.set_last_string(value)
    }

    fn entry_is_dir(&self, vfs_path: &str, index: i32) -> bool {
        perf::count("editor.tree.vfs.entry_is_dir_calls", 1);
        self.entry_at(vfs_path, index)
            .map(|entry| entry.kind == EntryKind::Dir)
            .unwrap_or(false)
    }

    fn is_dir(&self, vfs_path: &str) -> bool {
        if let Some(is_dir) = self.path_is_dir.borrow().get(vfs_path) {
            return *is_dir;
        }
        // `MiniFs::entries_path` always returns `Ok(...)` — including
        // for files and non-existent paths — so we cannot distinguish
        // files from directories with a direct `entries(...).is_ok()`
        // probe. Resolve via the parent listing instead: walking the
        // parent through `sorted_entries(...)` populates `path_is_dir`
        // for every sibling, after which the cache lookup tells us
        // exactly what kind this path is. Falling back to `false`
        // here is the safe choice — picks pointing at a real
        // directory will still expand the next time the tree view
        // surfaces them through its own listing, while picks pointing
        // at files no longer get silently swallowed by the editor's
        // "expand on click" branch.
        let path = Path::new(vfs_path);
        let parent = path
            .parent()
            .and_then(|p| p.to_str())
            .filter(|p| !p.is_empty())
            .unwrap_or("/");
        if vfs_path == "/" || path.file_name().is_none() {
            return true;
        }
        self.sorted_entries(parent);
        self.path_is_dir
            .borrow()
            .get(vfs_path)
            .copied()
            .unwrap_or(false)
    }

    fn is_expanded(&self, vfs_path: &str) -> bool {
        self.expanded_paths.borrow().contains(vfs_path)
    }

    fn toggle_expanded(&self, vfs_path: &str) {
        let mut expanded_paths = self.expanded_paths.borrow_mut();
        if !expanded_paths.insert(vfs_path.to_string()) {
            expanded_paths.remove(vfs_path);
        }
    }

    fn command_id(&self, vfs_path: &str) -> i32 {
        if let Some(id) = self.path_commands.borrow().get(vfs_path) {
            return *id;
        }
        let id = self.next_command_id.get();
        self.next_command_id.set(id.saturating_add(1));
        self.command_paths.borrow_mut().push(vfs_path.to_string());
        self.path_commands
            .borrow_mut()
            .insert(vfs_path.to_string(), id);
        id
    }

    fn command_path(&self, command_id: i32) -> &str {
        let value = command_id
            .checked_sub(1)
            .and_then(|idx| usize::try_from(idx).ok())
            .and_then(|idx| self.command_paths.borrow().get(idx).cloned())
            .unwrap_or_default();
        self.set_last_string(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_dir_distinguishes_files_from_directories_without_prior_listing() {
        // Regression: `MiniFs::entries_path` always returns `Ok(...)`,
        // even for files, so the old `entries(...).is_ok()` probe
        // reported every uncached path as a directory. The editor's
        // category-view leaves never pre-populated the cache, so
        // clicking them silently routed into the "toggle_expanded"
        // directory branch instead of opening a content tab.
        let root =
            std::env::temp_dir().join(format!("yaobow_vfs_is_dir_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("scene/Q01")).unwrap();
        fs::write(root.join("scene/Q01/Q01.bsp"), b"").unwrap();

        let mini = mini_fs::MiniFs::new(false).mount("/", mini_fs::LocalFs::new(&root));
        let vfs = VfsService::create(Rc::new(mini));
        let service = vfs
            .query_interface::<crate::comdef::services::IVfsService>()
            .unwrap();

        // File path — never previously listed.
        assert_eq!(service.is_dir("/scene/Q01/Q01.bsp"), false);
        // Directory path — never previously listed.
        assert_eq!(service.is_dir("/scene/Q01"), true);
        // Subsequent calls still return the same answers via cache.
        assert_eq!(service.is_dir("/scene/Q01/Q01.bsp"), false);
        assert_eq!(service.is_dir("/scene/Q01"), true);

        let _ = fs::remove_dir_all(&root);
    }
}
