//! `.ybpatch` reader, built on top of `ypk::YpkArchive`.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ypk::{SeekRead, YpkArchive};

use crate::error::{AssetProjectError, Result};
use crate::hash::ContentHash;
use crate::manifest::AssetChange;

use super::{
    PatchManifest, YBPATCH_FORMAT_VERSION, YBPATCH_MANIFEST_ENTRY, YBPATCH_MANIFEST_HASH_ENTRY,
    payload_entry_name,
};

/// Opens a `.ybpatch` file, verifying `manifest.json` against its
/// declared hash (`manifest.hash`) up front so a corrupted or
/// truncated patch fails fast with a clear error.
pub struct YbpatchReader {
    archive: YpkArchive,
    manifest: PatchManifest,
}

impl YbpatchReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| AssetProjectError::io(path, e))?;
        let reader: Arc<Mutex<dyn SeekRead + Send + Sync>> = Arc::new(Mutex::new(file));
        Self::from_reader(reader)
    }

    pub fn from_bytes(bytes: Arc<[u8]>) -> Result<Self> {
        let reader: Arc<Mutex<dyn SeekRead + Send + Sync>> =
            Arc::new(Mutex::new(std::io::Cursor::new(bytes)));
        Self::from_reader(reader)
    }

    fn from_reader(reader: Arc<Mutex<dyn SeekRead + Send + Sync>>) -> Result<Self> {
        let mut archive =
            YpkArchive::load(reader).map_err(|e| AssetProjectError::Ypk(e.to_string()))?;

        let manifest_bytes = read_entry(&mut archive, YBPATCH_MANIFEST_ENTRY)?;
        let manifest_hash_bytes = read_entry(&mut archive, YBPATCH_MANIFEST_HASH_ENTRY)?;
        let manifest_hash_str = String::from_utf8_lossy(&manifest_hash_bytes)
            .trim()
            .to_string();
        let expected_hash = ContentHash::from_hex(&manifest_hash_str)
            .ok_or_else(|| AssetProjectError::InvalidHash(manifest_hash_str.clone()))?;

        let actual_hash = ContentHash::of(&manifest_bytes);
        if actual_hash != expected_hash {
            return Err(AssetProjectError::HashMismatch {
                path: YBPATCH_MANIFEST_ENTRY.to_string(),
                expected: expected_hash.to_hex(),
                actual: actual_hash.to_hex(),
            });
        }

        let manifest: PatchManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| AssetProjectError::json(YBPATCH_MANIFEST_ENTRY, e))?;

        if manifest.format_version > YBPATCH_FORMAT_VERSION {
            return Err(AssetProjectError::UnsupportedManifestVersion {
                found: manifest.format_version,
                supported: YBPATCH_FORMAT_VERSION,
            });
        }

        Ok(Self { archive, manifest })
    }

    pub fn manifest(&self) -> &PatchManifest {
        &self.manifest
    }

    /// Reads and verifies one change's payload against its declared
    /// `payload.content_hash`.
    pub fn read_payload(&mut self, change: &AssetChange) -> Result<Vec<u8>> {
        let entry_name = payload_entry_name(&change.target_package, &change.package_internal_path);
        let data = read_entry(&mut self.archive, &entry_name)?;

        let actual = ContentHash::of(&data);
        if actual != change.payload.content_hash {
            return Err(AssetProjectError::HashMismatch {
                path: entry_name,
                expected: change.payload.content_hash.to_hex(),
                actual: actual.to_hex(),
            });
        }

        Ok(data)
    }

    /// Reads and verifies every payload declared in the manifest.
    /// Useful as a standalone integrity check before installing a
    /// patch (or before publishing one — see [`super::YbpatchWriter::finish`]).
    pub fn verify_all(&mut self) -> Result<()> {
        let changes = self.manifest.changes.clone();
        for change in &changes {
            self.read_payload(change)?;
        }
        Ok(())
    }
}

fn read_entry(archive: &mut YpkArchive, name: &str) -> Result<Vec<u8>> {
    let mut file = archive.open(name).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AssetProjectError::MissingPatchEntry(name.to_string())
        } else {
            AssetProjectError::io(Path::new(name), e)
        }
    })?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| AssetProjectError::io(Path::new(name), e))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ypk::YpkWriter;

    #[test]
    fn rejects_manifest_with_unsafe_paths() {
        let root = std::env::temp_dir().join(format!(
            "asset-project-unsafe-patch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        for (index, unsafe_path) in [
            "../evil.cpk",
            "/absolute.cpk",
            "C:/absolute.cpk",
            "scene//evil.cpk",
        ]
        .into_iter()
        .enumerate()
        {
            let path = root.join(format!("{index}.ybpatch"));
            let manifest = serde_json::json!({
                "format_version": 1,
                "patch_id": "00000000-0000-0000-0000-000000000000",
                "created_at": 0,
                "target_game": "pal3",
                "base_project_version": 1,
                "package_fingerprints": [{
                    "target_package": unsafe_path,
                    "base_hash": ContentHash::of(b"base"),
                }],
                "changes": [],
            });
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            let manifest_hash = ContentHash::of(&manifest_bytes);
            let file = File::create(&path).unwrap();
            let mut writer = YpkWriter::new(Box::new(file)).unwrap();
            writer
                .write_file(YBPATCH_MANIFEST_ENTRY, &manifest_bytes)
                .unwrap();
            writer
                .write_file(
                    YBPATCH_MANIFEST_HASH_ENTRY,
                    manifest_hash.to_hex().as_bytes(),
                )
                .unwrap();
            writer.finish().unwrap();

            assert!(YbpatchReader::open(&path).is_err(), "{unsafe_path}");
        }
    }
}
