//! Project-aware preview overlay mounted into the editor's `MiniFs`.
//!
//! `mini_fs::MiniFs::mount` takes `self` by value (`fn mount(mut self, ...)
//! -> Self`), and by the time an opened game's VFS reaches
//! `IEditorHostContext`/`IPreviewerHub`/`IResourceManager` it is already
//! shared behind an `Rc<MiniFs>` cloned into several long-lived
//! services (see `directors::app_service::AppService::open_game`).
//! There is no way to get the owned `MiniFs` back out of an `Rc` with
//! more than one strong reference, so a project opened *after* the
//! game's VFS was built cannot be layered in via a second `mount()`
//! call ("live remount").
//!
//! Instead, [`ProjectOverlayStore`] is mounted exactly once, when the
//! game's VFS is first assembled (`AppService::open_game`), at the
//! same priority tier as any other package mount — but it reads
//! through a [`SharedOverlayIndex`] (`Rc<RefCell<...>>`) that a later
//! opened/edited [`crate::services::project_service::ProjectService`]
//! mutates in place. No remount is ever required: staging or removing
//! a change just updates the shared index, and the next `vfs.open(...)`
//! (from a previewer, the resource browser, etc.) observes it
//! immediately, falling through to the untouched base package mount
//! whenever the overlay has nothing staged for a given path.
//!
//! This is the "project-aware overlay store/path read layer" mandated
//! as a fallback when live remounting isn't possible — except it
//! doubles as the primary mechanism, since interior mutability sidesteps
//! the remount problem entirely rather than merely working around it.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use asset_project::{ContentHash, PayloadStore};
use mini_fs::{Entries, Entry, EntryKind, Store, UserFile};

/// Shared, mutable index from absolute vfs path (e.g.
/// `/scene/q01/q01.scn`) to the content hash of the project change
/// currently staged there, plus the payload store to read the actual
/// bytes from. Cloned (as an `Rc`) between the mounted
/// [`ProjectOverlayStore`] and the owning `ProjectService`.
pub type SharedOverlayIndex = Rc<std::cell::RefCell<OverlayIndexData>>;

#[derive(Default)]
pub struct OverlayIndexData {
    payload_store: Option<PayloadStore>,
    entries: HashMap<PathBuf, ContentHash>,
}

impl OverlayIndexData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the overlay's entire contents (called on project
    /// open/close/change-mutation). `payload_store` is `None` when no
    /// project is active, which also acts as a hard "serve nothing"
    /// switch even if `entries` were somehow left stale.
    pub fn reset(
        &mut self,
        payload_store: Option<PayloadStore>,
        entries: HashMap<PathBuf, ContentHash>,
    ) {
        self.payload_store = payload_store;
        self.entries = entries;
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

pub fn new_shared_overlay_index() -> SharedOverlayIndex {
    Rc::new(std::cell::RefCell::new(OverlayIndexData::new()))
}

/// Normalizes a `mini_fs` path (which, depending on the mount point a
/// `Store` is attached at, may or may not carry a leading `/`) to an
/// absolute path so overlay index lookups don't have to care which
/// form `MiniFs` happens to hand them.
fn to_absolute(path: &Path) -> PathBuf {
    Path::new("/").join(path)
}

/// Owned in-memory file backing an overlay read. Bytes are held in an
/// `Arc<[u8]>` (rather than the crate-private `mini_fs::RamFile`) so
/// cloning the handle for a re-open is cheap and so the type can
/// satisfy `mini_fs::UserFile` (`Any + Read + Seek + Send`) from
/// outside the `mini_fs` crate.
pub struct OverlayFile(Cursor<Arc<[u8]>>);

impl OverlayFile {
    fn new(bytes: Vec<u8>) -> Self {
        Self(Cursor::new(Arc::from(bytes)))
    }
}

impl Read for OverlayFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for OverlayFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl UserFile for OverlayFile {}

/// `mini_fs::Store` that serves staged project-change payloads ahead
/// of the base asset packages it's mounted alongside. See the module
/// docs for why this achieves a "live" overlay without ever needing to
/// rebuild/remount the `MiniFs` a project is layered onto.
pub struct ProjectOverlayStore {
    index: SharedOverlayIndex,
}

impl ProjectOverlayStore {
    pub fn new(index: SharedOverlayIndex) -> Self {
        Self { index }
    }
}

impl Store for ProjectOverlayStore {
    type File = OverlayFile;

    fn open_path(&self, path: &Path) -> io::Result<Self::File> {
        let abs = to_absolute(path);
        let data = self.index.borrow();
        let hash = *data
            .entries
            .get(&abs)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let store = data
            .payload_store
            .as_ref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let bytes = store
            .get(hash)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(OverlayFile::new(bytes))
    }

    fn entries_path(&self, path: &Path) -> io::Result<Entries<'_>> {
        let dir = to_absolute(path);
        let data = self.index.borrow();

        // Dedup by immediate child name, escalating to `Dir` if any
        // tracked path implies a deeper nesting under that name.
        let mut children: HashMap<OsString, EntryKind> = HashMap::new();
        for key in data.entries.keys() {
            let Ok(rel) = key.strip_prefix(&dir) else {
                continue;
            };
            let mut components = rel.components();
            let Some(first) = components.next() else {
                continue;
            };
            let name = first.as_os_str().to_owned();
            let kind = if components.next().is_some() {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            children
                .entry(name)
                .and_modify(|existing| {
                    if kind == EntryKind::Dir {
                        *existing = EntryKind::Dir;
                    }
                })
                .or_insert(kind);
        }

        let entries: Vec<io::Result<Entry>> = children
            .into_iter()
            .map(|(name, kind)| Ok(Entry { name, kind }))
            .collect();
        Ok(Entries::new(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mini_fs::{MiniFs, RamFs, StoreExt};

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!("{name}-{}-{}", std::process::id(), unique_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn stage(index: &SharedOverlayIndex, store: PayloadStore, path: &str, bytes: &[u8]) {
        let hash = store.put(bytes).unwrap();
        let mut data = index.borrow_mut();
        let mut entries = std::mem::take(&mut data.entries);
        entries.insert(PathBuf::from(path), hash);
        data.reset(Some(store), entries);
    }

    #[test]
    fn overlay_serves_staged_bytes_and_falls_through_for_untracked_paths() {
        let dir = scratch_dir("overlay-basic");
        let store = PayloadStore::new(dir.join("payloads"));
        let index = new_shared_overlay_index();
        stage(&index, store, "/scene/q01/q01.scn", b"staged bytes");

        let mut base = RamFs::new();
        base.touch("scene/q01/q01.scn", b"base bytes".to_vec());
        base.touch("scene/other.txt", b"untouched".to_vec());

        let vfs = MiniFs::new(true)
            .mount("/", base)
            .mount("/", ProjectOverlayStore::new(index));

        let mut file = vfs.open("/scene/q01/q01.scn").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"staged bytes");

        let mut file = vfs.open("/scene/other.txt").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"untouched");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overlay_returns_not_found_for_untracked_path_with_no_base_mount() {
        let index = new_shared_overlay_index();
        let vfs = MiniFs::new(true).mount("/", ProjectOverlayStore::new(index));
        assert!(vfs.open("/nope.txt").is_err());
    }

    #[test]
    fn overlay_lists_staged_entries_alongside_base_entries() {
        let dir = scratch_dir("overlay-entries");
        let store = PayloadStore::new(dir.join("payloads"));
        let index = new_shared_overlay_index();
        stage(&index, store, "/scene/new_file.scn", b"new");

        let mut base = RamFs::new();
        base.touch("scene/q01.scn", b"base".to_vec());

        let vfs = MiniFs::new(true)
            .mount("/", base)
            .mount("/", ProjectOverlayStore::new(index));

        let mut names: Vec<String> = vfs
            .entries("/scene")
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.name.to_string_lossy().to_string())
            .collect();
        names.sort();
        // `RamFs` reports entry names relative to the store root
        // ("scene/q01.scn"); our overlay reports bare basenames
        // ("new_file.scn"). Both are expected to show up in the
        // aggregated listing.
        assert!(names.iter().any(|n| n.contains("q01.scn")));
        assert!(names.iter().any(|n| n.contains("new_file.scn")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
