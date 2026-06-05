//! Engine-side asset / virtual-filesystem facade.
//!
//! `radiance::asset` lets the engine and engine-side hosts mount asset
//! sources (local folders, `.zip` files, `.ypk` bundles, or any
//! `mini_fs::Store`) and read files from them without importing
//! `mini_fs` directly. This is the surface engine consumers should
//! prefer.
//!
//! ## Why this lives in `radiance`
//!
//! Historically all VFS plumbing lived in the `packfs` crate alongside
//! the game-specific archive formats (`.cpk`, `.pkg`, `.zpk`, ...).
//! Those formats stay in `packfs` because they're tied to specific
//! Softstar/Aurogon titles. But the engine itself needs a VFS handle
//! and a way to ship its own bundle format, so we host the generic
//! pieces here: the `AssetManager` facade, `MemoryFile` /
//! `StreamingFile`, and the `.ypk` bundle format (which has no
//! game-specific reverse-engineering and is yaobow's own).
//!
//! ## Mounting model
//!
//! `mini_fs::MiniFs::mount` is `self`-by-value (builder style). To
//! expose `&self`-mounting through the `AssetManager` we hold an
//! `Option<MiniFs>` inside a `RefCell` and rotate it through each
//! mount. All `mount_*` calls are expected to happen at app boot on
//! the single thread that constructed the engine, before any
//! concurrent `open`/`read` traffic — same lifecycle as today, where
//! `packfs::init_virtual_fs` builds the entire `MiniFs` up front.
//!
//! ## What this *doesn't* do
//!
//! - No asset caching, no hot-reload, no type-aware decoders. Those
//!   would belong in a future asset-pipeline pass; right now this is
//!   just a thin facade over `mini_fs::MiniFs` + the file helpers that
//!   used to live in `yaobow/common::store_ext`.
//! - No `packfs::init_virtual_fs` replacement — that walker still owns
//!   game-format mounting.

use std::cell::{Ref, RefCell};
use std::error::Error;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use encoding::{DecoderTrap, Encoding};
use mini_fs::{LocalFs, MiniFs, Store, StoreExt, ZipFs};

pub mod file;
pub mod seek_traits;
pub mod ypk;

pub use mini_fs::{Entries, Entry, EntryKind, File};

use self::ypk::YpkFs;

pub struct AssetManager {
    /// `Option` so we can swap the inner `MiniFs` through the
    /// by-value-`self` `mount(...)` builder on each mount call.
    /// Invariant: outside of a `mutate_vfs(...)` body the option is
    /// always `Some`. `mutate_vfs` is panic-safe — see [`MutateGuard`]
    /// below.
    vfs: RefCell<Option<MiniFs>>,
    /// Stored so `mutate_vfs`'s panic guard can reinstate a fresh,
    /// empty `MiniFs` of the same case-sensitivity if the mount
    /// closure unwinds. Without this we have no way to consult the
    /// original setting (`MiniFs::case_sensitive` is private upstream).
    case_sensitive: bool,
}

/// Reinstates a fresh empty `MiniFs` into the parent `AssetManager`
/// on drop if mutation didn't complete (i.e. the mount closure
/// panicked). Without the guard a panic in a mount closure would
/// leave `vfs = None`, and the next `mini_fs()`/`open(...)` call
/// would trip the invariant `expect()`.
///
/// Losing prior mounts on panic is the lesser evil: the alternative
/// would be propagating the panic with a permanently-poisoned
/// AssetManager.
struct MutateGuard<'a> {
    slot: std::cell::RefMut<'a, Option<MiniFs>>,
    case_sensitive: bool,
    completed: bool,
}

impl Drop for MutateGuard<'_> {
    fn drop(&mut self) {
        if !self.completed && self.slot.is_none() {
            *self.slot = Some(MiniFs::new(self.case_sensitive));
        }
    }
}

impl AssetManager {
    /// Case-insensitive `MiniFs` — matches the legacy
    /// `packfs::init_virtual_fs` behaviour and what game paths
    /// (typically baked at varying case in `.cpk`/`.ypk` entry
    /// tables) need.
    pub fn new() -> Rc<Self> {
        Self::with_case_sensitivity(false)
    }

    pub fn new_case_sensitive() -> Rc<Self> {
        Self::with_case_sensitivity(true)
    }

    fn with_case_sensitivity(case_sensitive: bool) -> Rc<Self> {
        Rc::new(Self {
            vfs: RefCell::new(Some(MiniFs::new(case_sensitive))),
            case_sensitive,
        })
    }

    /// Wrap an externally-constructed `MiniFs` (typically the result
    /// of `packfs::init_virtual_fs(...)`) into an `AssetManager`.
    /// `case_sensitive` must match how that `MiniFs` was created —
    /// `mini_fs::MiniFs` doesn't expose its own case-sensitivity, so
    /// passing the wrong value would only affect panic-recovery
    /// behaviour in `mutate_vfs`, not lookups.
    pub fn from_mini_fs(vfs: MiniFs, case_sensitive: bool) -> Rc<Self> {
        Rc::new(Self {
            vfs: RefCell::new(Some(vfs)),
            case_sensitive,
        })
    }

    fn mutate_vfs<F>(&self, f: F)
    where
        F: FnOnce(MiniFs) -> MiniFs,
    {
        let mut guard = MutateGuard {
            slot: self.vfs.borrow_mut(),
            case_sensitive: self.case_sensitive,
            completed: false,
        };
        let prev = guard
            .slot
            .take()
            .expect("AssetManager::vfs invariant: Some outside mutate_vfs");
        let next = f(prev);
        *guard.slot = Some(next);
        guard.completed = true;
    }

    /// Borrow the inner `MiniFs` for callers that still need to pass it
    /// to APIs typed against `mini_fs` directly (notably the script
    /// VFS service). Prefer the `AssetManager` helpers over this.
    pub fn mini_fs(&self) -> Ref<'_, MiniFs> {
        Ref::map(self.vfs.borrow(), |slot| {
            slot.as_ref()
                .expect("AssetManager::vfs invariant: Some outside mutate_vfs")
        })
    }

    // -- Mounting ----------------------------------------------------

    pub fn mount_local<P, V>(&self, vfs_path: V, local_path: P)
    where
        P: AsRef<Path>,
        V: Into<PathBuf>,
    {
        let store = LocalFs::new(local_path.as_ref());
        self.mutate_vfs(|vfs| vfs.mount(vfs_path.into(), store));
    }

    pub fn mount_zip<P, V>(&self, vfs_path: V, zip_path: P) -> io::Result<()>
    where
        P: AsRef<Path>,
        V: Into<PathBuf>,
    {
        let store = ZipFs::open(zip_path.as_ref())?;
        self.mutate_vfs(|vfs| vfs.mount(vfs_path.into(), store));
        Ok(())
    }

    pub fn mount_ypk<P, V>(&self, vfs_path: V, ypk_path: P) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
        V: Into<PathBuf>,
    {
        let store = YpkFs::new(ypk_path.as_ref())?;
        self.mutate_vfs(|vfs| vfs.mount(vfs_path.into(), store));
        Ok(())
    }

    pub fn mount_store<S, T, V>(&self, vfs_path: V, store: S)
    where
        S: Store<File = T> + 'static,
        T: Into<File> + 'static,
        V: Into<PathBuf>,
    {
        self.mutate_vfs(|vfs| vfs.mount(vfs_path.into(), store));
    }

    // -- Reading -----------------------------------------------------

    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        self.mini_fs().open(path)
    }

    /// Try `path`; on miss, retry once per `fallback_ext` swapping the
    /// extension. Returns the first successful open, or the original
    /// error if every attempt missed. Matches the legacy
    /// `common::StoreExt2::open_with_fallback` semantics.
    pub fn open_with_fallback<P: AsRef<Path>>(
        &self,
        path: P,
        fallback_ext: &[&str],
    ) -> io::Result<File> {
        let res = self.mini_fs().open(path.as_ref());
        if res.is_err() {
            for ext in fallback_ext {
                let new_path = path.as_ref().with_extension(ext);
                let attempt = self.mini_fs().open(new_path);
                if attempt.is_ok() {
                    return attempt;
                }
            }
        }
        res
    }

    pub fn try_open_files<P: AsRef<Path>>(&self, paths: &[P]) -> io::Result<File> {
        for p in paths {
            let attempt: io::Result<File> = self.mini_fs().open(p);
            if attempt.is_ok() {
                return attempt;
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "File not found"))
    }

    pub fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let file = self.open(path)?;
        let mut bytes = Vec::new();
        BufReader::new(file).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub fn read_to_end_from_gbk<P: AsRef<Path>>(&self, path: P) -> Result<String, Box<dyn Error>> {
        let data = self.read_to_end(path)?;
        Ok(encoding::all::GBK.decode(&data, DecoderTrap::Ignore)?)
    }

    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        self.mini_fs().open(path).is_ok()
    }

    pub fn entries<P: AsRef<Path>>(&self, path: P) -> io::Result<Entries<'_>> {
        // `entries_path` borrows from the underlying store; the
        // returned `Entries` holds that borrow alive for as long as
        // it lives. We project the borrow through the `RefCell` so
        // the `Ref<MiniFs>` guard is kept for the iterator's lifetime
        // via `Entries`' own lifetime tying.
        //
        // Note: `Ref` cannot be re-projected to yield a borrow with a
        // shorter lifetime that also extends past the `Ref`'s scope,
        // so we deliberately leak the borrow into a `Box<dyn Iterator>`
        // by collecting eagerly. The legacy `StoreExt2` callers all
        // either iterate to completion immediately or `.collect()`
        // anyway.
        let collected: Vec<io::Result<Entry>> = {
            let vfs = self.mini_fs();
            let it = vfs.entries(path.as_ref())?;
            it.collect()
        };
        Ok(Entries::new(collected.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::asset::seek_traits::SeekWrite;
    use crate::asset::ypk::YpkWriter;

    fn unique_tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("yaobow_asset_{}_{}", std::process::id(), name))
    }

    #[test]
    fn ypk_round_trip_through_asset_manager() {
        // Round-trip a `.ypk` written via `YpkWriter`, mounted via
        // `AssetManager::mount_ypk`, and read back through
        // `read_to_end`. Locks the public surface against regressions
        // in the asset module + ypk relocation.
        let ypk_path = unique_tmp("round_trip.ypk");
        let _ = fs::remove_file(&ypk_path);

        // YpkWriter takes a Box<dyn SeekWrite>; we use a real file
        // here so the same path we hand to `mount_ypk` is the one
        // that received the entries (avoids the Cursor-downcast
        // dance YpkWriter::finish would otherwise force).
        {
            let f = fs::File::create(&ypk_path).unwrap();
            let writer: Box<dyn SeekWrite> = Box::new(f);
            let mut ypk = YpkWriter::new(writer).unwrap();
            ypk.write_file("hello/world.txt", b"hello, world!").unwrap();
            ypk.write_file("foo/bar.bin", &[1u8, 2, 3, 4, 5]).unwrap();
            ypk.finish().unwrap();
        }

        let assets = AssetManager::new();
        assets.mount_ypk("/bundle", &ypk_path).unwrap();

        assert!(assets.exists("/bundle/hello/world.txt"));
        assert_eq!(
            assets.read_to_end("/bundle/hello/world.txt").unwrap(),
            b"hello, world!".to_vec()
        );
        assert_eq!(
            assets.read_to_end("/bundle/foo/bar.bin").unwrap(),
            vec![1u8, 2, 3, 4, 5]
        );
        assert!(!assets.exists("/bundle/does/not/exist"));

        let _ = fs::remove_file(&ypk_path);
    }

    #[test]
    fn mount_local_and_read_back() {
        let root = unique_tmp("local_root");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/a.txt"), b"alpha").unwrap();

        let assets = AssetManager::new();
        assets.mount_local("/data", &root);

        assert!(assets.exists("/data/sub/a.txt"));
        assert_eq!(assets.read_to_end("/data/sub/a.txt").unwrap(), b"alpha");
        assert!(!assets.exists("/data/missing"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn open_with_fallback_picks_alt_extension() {
        let root = unique_tmp("fallback_root");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("song.ogg"), b"OGG!").unwrap();

        let assets = AssetManager::new();
        assets.mount_local("/audio", &root);

        let mut file = assets
            .open_with_fallback("/audio/song.wav", &["ogg"])
            .unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"OGG!");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn entries_iteration_allows_concurrent_open_calls() {
        // Regression for the rubber-duck B finding: `entries()` must
        // not hold an outstanding `Ref<MiniFs>` borrow across
        // iteration, so callers can `open(...)` inside the loop
        // without tripping `RefCell` re-borrow rules. Eager
        // collection inside `entries()` is what makes this work.
        let root = unique_tmp("entries_root");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("d")).unwrap();
        fs::write(root.join("d/a.txt"), b"A").unwrap();
        fs::write(root.join("d/b.txt"), b"BB").unwrap();

        let assets = AssetManager::new();
        assets.mount_local("/m", &root);

        let mut total = 0usize;
        for entry in assets.entries("/m/d").unwrap() {
            let entry = entry.unwrap();
            // `entry.name` from `LocalFs` is the path relative to the
            // mount root (e.g. "d/a.txt"), not the bare leaf — same
            // behaviour `radiance_scripting::services::VfsService`
            // works around with `PathBuf::file_name(...)`. Calling
            // `open(...)` would borrow the inner `MiniFs` again; if
            // `entries()` held a Ref across iteration this would
            // panic. Eager collection inside `entries()` is what
            // makes it work.
            let leaf = Path::new(&entry.name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap();
            let path = format!("/m/d/{leaf}");
            let bytes = assets.read_to_end(&path).unwrap();
            total += bytes.len();
        }
        assert_eq!(total, 3); // 1 + 2

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ypk_directory_listing_returns_immediate_children() {
        // Locks YpkFs::entries_path: was `unimplemented!()` before
        // the move into radiance and would have panicked any caller
        // listing a ypk-mounted directory through `AssetManager`.
        let ypk_path = unique_tmp("listing.ypk");
        let _ = fs::remove_file(&ypk_path);

        {
            let f = fs::File::create(&ypk_path).unwrap();
            let writer: Box<dyn SeekWrite> = Box::new(f);
            let mut ypk = YpkWriter::new(writer).unwrap();
            ypk.write_file("a.txt", b"a").unwrap();
            ypk.write_file("dir1/b.txt", b"b").unwrap();
            ypk.write_file("dir1/c.txt", b"c").unwrap();
            ypk.write_file("dir1/sub/d.txt", b"d").unwrap();
            ypk.write_file("dir2/e.txt", b"e").unwrap();
            ypk.finish().unwrap();
        }

        let assets = AssetManager::new();
        assets.mount_ypk("/b", &ypk_path).unwrap();

        let root: Vec<_> = assets
            .entries("/b")
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| (e.name.to_string_lossy().to_string(), e.kind))
            .collect();
        // Root should expose `a.txt` (file), `dir1` (dir), `dir2` (dir)
        // — and nothing deeper.
        let names: Vec<_> = root.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"dir1"));
        assert!(names.contains(&"dir2"));
        assert!(!names.iter().any(|n| n.contains('/')));

        let dir1: Vec<_> = assets
            .entries("/b/dir1")
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| (e.name.to_string_lossy().to_string(), e.kind))
            .collect();
        let dir1_names: Vec<_> = dir1.iter().map(|(n, _)| n.as_str()).collect();
        assert!(dir1_names.contains(&"b.txt"));
        assert!(dir1_names.contains(&"c.txt"));
        assert!(dir1_names.contains(&"sub"));
        let sub_kind = dir1
            .iter()
            .find(|(n, _)| n == "sub")
            .map(|(_, k)| *k)
            .unwrap();
        assert_eq!(sub_kind, EntryKind::Dir);

        let _ = fs::remove_file(&ypk_path);
    }
}
