//! Atomic file persistence used by both the project manifest and the
//! installation journal.
//!
//! The write sequence is: write full contents to a sibling temp file,
//! `fsync` it, then `rename` it over the destination. `rename` is
//! atomic within the same filesystem on both POSIX and Windows, so a
//! reader (or a crash mid-write) can never observe a half-written
//! manifest/journal — it either sees the old complete file or the new
//! complete file, never a partial one.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AssetProjectError, Result};

/// Atomically writes `data` to `path`, creating parent directories as
/// needed. Safe to call concurrently from multiple processes writing
/// the *same* destination path: each writer uses its own uniquely
/// named temp file, so writers never corrupt each other's temp data,
/// and the final `rename` leaves whichever writer renamed last as the
/// visible result.
pub fn atomic_write(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
    let path = path.as_ref();
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir).map_err(|e| AssetProjectError::io(dir, e))?;

    let tmp_path = temp_sibling_path(path);
    write_temp(&tmp_path, data)?;

    fs::rename(&tmp_path, path).map_err(|e| {
        // Best-effort cleanup so a failed rename doesn't leave the temp
        // file behind forever.
        let _ = fs::remove_file(&tmp_path);
        AssetProjectError::io(path, e)
    })?;

    // Best-effort: fsync the containing directory so the rename entry
    // itself is durable. Not all platforms support opening a directory
    // for this purpose, so failures here are intentionally ignored.
    if let Ok(dir_file) = File::open(dir) {
        let _ = dir_file.sync_all();
    }

    Ok(())
}

fn write_temp(tmp_path: &Path, data: &[u8]) -> Result<()> {
    let mut file = File::create(tmp_path).map_err(|e| AssetProjectError::io(tmp_path, e))?;
    file.write_all(data)
        .map_err(|e| AssetProjectError::io(tmp_path, e))?;
    file.sync_all()
        .map_err(|e| AssetProjectError::io(tmp_path, e))?;
    Ok(())
}

/// Builds a sibling temp-file path next to `path` with a unique name
/// (pid + nanosecond timestamp), so concurrent writers targeting the
/// same destination never collide on their own temp files. Exposed as
/// `pub(crate)` so `patch::writer::YapatchWriter` can use the same
/// write-temp/verify/rename sequence for publishing `.yapatch` files.
pub(crate) fn temp_sibling_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("asset_project");
    let unique = format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        unix_now_nanos()
    );
    path.with_file_name(unique)
}

fn unix_now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

/// Unix-epoch seconds, used for manifest/journal timestamps. Kept as a
/// plain `u64` (rather than pulling in a datetime crate) since this
/// crate only needs monotonic-ish, JSON-friendly timestamps.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Reads a file's full contents, wrapping I/O errors with the path.
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    fs::read(path).map_err(|e| AssetProjectError::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!(
                "{}-{}-{}",
                name,
                std::process::id(),
                unix_now_nanos()
            ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_and_overwrites_atomically() {
        let dir = scratch_dir("atomic-write");
        let path = dir.join("file.txt");

        atomic_write(&path, b"first").unwrap();
        assert_eq!(read_file(&path).unwrap(), b"first");

        atomic_write(&path, b"second, longer content").unwrap();
        assert_eq!(read_file(&path).unwrap(), b"second, longer content");

        // No leftover temp files.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = scratch_dir("atomic-write-nested");
        let path = dir.join("a/b/c/file.txt");

        atomic_write(&path, b"nested").unwrap();
        assert_eq!(read_file(&path).unwrap(), b"nested");

        let _ = fs::remove_dir_all(&dir);
    }
}
