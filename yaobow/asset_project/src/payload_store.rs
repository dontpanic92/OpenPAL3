//! Content-addressed storage for converted asset payload bytes.
//!
//! Each payload is written under `<root>/<hash[0..2]>/<hash>` (a
//! git-style two-level fan-out keyed by `ContentHash`), so identical
//! converted bytes referenced by multiple `AssetChange`s are stored
//! exactly once. Writes go through [`crate::atomic::atomic_write`], so
//! a crash mid-import can never leave a corrupt object visible under
//! its final hash-named path — either the object is fully present or
//! not present at all.

use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::error::{AssetProjectError, Result};
use crate::hash::ContentHash;

pub struct PayloadStore {
    root: PathBuf,
}

impl PayloadStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stores `data` and returns its content hash. If an object with
    /// the same hash is already present, this is a cheap no-op rather
    /// than an error — that's the point of content-addressed dedup.
    pub fn put(&self, data: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::of(data);
        let path = self.path_for(hash);
        if !path.exists() {
            atomic_write(&path, data)?;
        }
        Ok(hash)
    }

    /// Reads back a previously stored payload, re-verifying its bytes
    /// against `hash` so silent on-disk corruption is caught instead of
    /// silently handed to a caller.
    pub fn get(&self, hash: ContentHash) -> Result<Vec<u8>> {
        let path = self.path_for(hash);
        let data = std::fs::read(&path).map_err(|e| AssetProjectError::io(&path, e))?;
        let actual = ContentHash::of(&data);
        if actual != hash {
            return Err(AssetProjectError::HashMismatch {
                path: path.display().to_string(),
                expected: hash.to_hex(),
                actual: actual.to_hex(),
            });
        }
        Ok(data)
    }

    pub fn contains(&self, hash: ContentHash) -> bool {
        self.path_for(hash).exists()
    }

    pub fn path_for(&self, hash: ContentHash) -> PathBuf {
        let hex = hash.to_hex();
        let (prefix, rest) = hex.split_at(2);
        self.root.join(prefix).join(rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!(
                "{}-{}-{}",
                name,
                std::process::id(),
                crate::atomic::unix_now()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn put_then_get_round_trips() {
        let dir = scratch_dir("payload-store");
        let store = PayloadStore::new(&dir);

        let hash = store.put(b"hello payload").unwrap();
        assert!(store.contains(hash));
        assert_eq!(store.get(hash).unwrap(), b"hello payload");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn put_is_idempotent_for_identical_content() {
        let dir = scratch_dir("payload-store-dedup");
        let store = PayloadStore::new(&dir);

        let hash_a = store.put(b"same bytes").unwrap();
        let hash_b = store.put(b"same bytes").unwrap();
        assert_eq!(hash_a, hash_b);
        assert_eq!(store.get(hash_a).unwrap(), b"same bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_detects_on_disk_corruption() {
        let dir = scratch_dir("payload-store-corruption");
        let store = PayloadStore::new(&dir);

        let hash = store.put(b"original bytes").unwrap();
        std::fs::write(store.path_for(hash), b"corrupted!!").unwrap();

        let err = store.get(hash).unwrap_err();
        assert!(matches!(err, AssetProjectError::HashMismatch { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_missing_object_is_an_io_error() {
        let dir = scratch_dir("payload-store-missing");
        let store = PayloadStore::new(&dir);

        let missing_hash = ContentHash::of(b"never stored");
        let err = store.get(missing_hash).unwrap_err();
        assert!(matches!(err, AssetProjectError::Io { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
