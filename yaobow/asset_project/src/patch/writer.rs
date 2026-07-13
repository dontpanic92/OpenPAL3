//! `.yapatch` writer, built on top of `radiance::asset::ypk::YpkWriter`.
//!
//! Publishing is atomic: [`YapatchWriter::finish`] writes the archive
//! at a sibling temp path, closes it, re-opens it through
//! [`super::YapatchReader`] to verify the manifest hash and every
//! payload hash it just wrote, and only then renames the verified temp
//! file onto the destination `path` passed to
//! [`YapatchWriter::create`]. If verification (or any earlier step)
//! fails, the temp file is removed and the destination path is left
//! completely untouched.

use std::fs::File;
use std::path::{Path, PathBuf};

use radiance::asset::ypk::YpkWriter;
use uuid::Uuid;

use crate::atomic::{temp_sibling_path, unix_now};
use crate::error::{AssetProjectError, Result};
use crate::hash::ContentHash;
use crate::manifest::AssetChange;

use super::{
    PackageFingerprint, PatchManifest, YAPATCH_FORMAT_VERSION, YAPATCH_MANIFEST_ENTRY,
    YAPATCH_MANIFEST_HASH_ENTRY, YapatchReader, payload_entry_name,
};

/// Incrementally packs asset changes into a `.yapatch` file.
///
/// ```ignore
/// let mut writer = YapatchWriter::create("update.yapatch", "pal3", base_project_version)?;
/// writer.add_package_fingerprint(fingerprint);
/// writer.add_change(change, &payload_bytes)?;
/// let manifest = writer.finish()?; // atomic publish: verified, then renamed into place
/// ```
pub struct YapatchWriter {
    ypk: YpkWriter,
    final_path: PathBuf,
    temp_path: PathBuf,
    target_game: String,
    base_project_version: u32,
    package_fingerprints: Vec<PackageFingerprint>,
    changes: Vec<AssetChange>,
}

impl YapatchWriter {
    /// Starts building a `.yapatch` that will be published to `path`
    /// once [`finish`](Self::finish) succeeds. Nothing is written to
    /// `path` itself until then — all writes go to a sibling temp file.
    pub fn create(
        path: impl AsRef<Path>,
        target_game: impl Into<String>,
        base_project_version: u32,
    ) -> Result<Self> {
        let final_path = path.as_ref().to_path_buf();

        if let Some(dir) = final_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| AssetProjectError::io(dir, e))?;
        }

        let temp_path = temp_sibling_path(&final_path);
        let file = File::create(&temp_path).map_err(|e| AssetProjectError::io(&temp_path, e))?;
        let ypk =
            YpkWriter::new(Box::new(file)).map_err(|e| AssetProjectError::Ypk(e.to_string()))?;

        Ok(Self {
            ypk,
            final_path,
            temp_path,
            target_game: target_game.into(),
            base_project_version,
            package_fingerprints: Vec::new(),
            changes: Vec::new(),
        })
    }

    /// Records the expected pre-patch state of a target package. The
    /// installer checks this against the actual target package before
    /// applying any of this patch's changes to it.
    pub fn add_package_fingerprint(&mut self, fingerprint: PackageFingerprint) {
        self.package_fingerprints.push(fingerprint);
    }

    /// Adds one change plus its payload bytes to the patch.
    ///
    /// `payload` must hash to `change.payload.content_hash` — this is
    /// checked up front so a stale/desynced `AssetChange` record is
    /// caught before it is ever written to disk, rather than surfacing
    /// as a confusing failure when the patch is later installed.
    pub fn add_change(&mut self, change: AssetChange, payload: &[u8]) -> Result<()> {
        let actual = ContentHash::of(payload);
        if actual != change.payload.content_hash {
            return Err(AssetProjectError::HashMismatch {
                path: change.package_internal_path.as_str().to_string(),
                expected: change.payload.content_hash.to_hex(),
                actual: actual.to_hex(),
            });
        }

        let entry_name = payload_entry_name(&change.target_package, &change.package_internal_path);
        self.ypk
            .write_file(&entry_name, payload)
            .map_err(|e| AssetProjectError::io(Path::new(&entry_name), e))?;
        self.changes.push(change);
        Ok(())
    }

    /// Number of changes added so far.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Finalizes the patch and atomically publishes it to the
    /// destination path given to [`create`](Self::create).
    ///
    /// Sequence: write the manifest entries into the temp `.ypk`,
    /// close it, re-open it through [`YapatchReader`] to verify the
    /// manifest hash and every payload hash it just wrote, then rename
    /// the verified temp file onto the destination. On any failure the
    /// temp file is removed and the destination is left untouched.
    pub fn finish(self) -> Result<PatchManifest> {
        // Captured before `self` is consumed below, so cleanup can run
        // regardless of which step inside `finish_and_verify` fails.
        let temp_path = self.temp_path.clone();

        match self.finish_and_verify() {
            Ok(manifest) => Ok(manifest),
            Err(err) => {
                let _ = std::fs::remove_file(&temp_path);
                Err(err)
            }
        }
    }

    /// Does the actual write/close/verify/rename work. Takes `self` by
    /// value (rather than by reference) purely so `self.ypk.finish()`
    /// can move the underlying writer out of the struct; `finish`
    /// above is the only caller and handles temp-file cleanup on error.
    fn finish_and_verify(mut self) -> Result<PatchManifest> {
        let manifest = PatchManifest {
            format_version: YAPATCH_FORMAT_VERSION,
            patch_id: Uuid::new_v4(),
            created_at: unix_now(),
            target_game: self.target_game,
            base_project_version: self.base_project_version,
            package_fingerprints: self.package_fingerprints,
            changes: self.changes,
        };

        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| AssetProjectError::json(PathBuf::from(YAPATCH_MANIFEST_ENTRY), e))?;
        let manifest_hash = ContentHash::of(&manifest_bytes);

        self.ypk
            .write_file(YAPATCH_MANIFEST_ENTRY, &manifest_bytes)
            .map_err(|e| AssetProjectError::io(Path::new(YAPATCH_MANIFEST_ENTRY), e))?;
        self.ypk
            .write_file(
                YAPATCH_MANIFEST_HASH_ENTRY,
                manifest_hash.to_hex().as_bytes(),
            )
            .map_err(|e| AssetProjectError::io(Path::new(YAPATCH_MANIFEST_HASH_ENTRY), e))?;
        self.ypk
            .finish()
            .map_err(|e| AssetProjectError::Ypk(e.to_string()))?;

        // Re-open and fully verify the temp file before it's ever
        // visible at `final_path`: a bug in the writer or an
        // interrupted flush should never result in an unverified (or
        // partially written) file landing at the destination.
        let mut reader = YapatchReader::open(&self.temp_path)?;
        reader.verify_all()?;
        drop(reader);

        std::fs::rename(&self.temp_path, &self.final_path)
            .map_err(|e| AssetProjectError::io(&self.final_path, e))?;

        Ok(manifest)
    }
}

/// One-shot convenience wrapper around [`YapatchWriter`] for callers
/// that already have every fingerprint/change/payload in hand and
/// don't need incremental control. Goes through the exact same
/// write-temp/verify/rename publish sequence as
/// [`YapatchWriter::finish`].
pub fn publish(
    path: impl AsRef<Path>,
    target_game: impl Into<String>,
    base_project_version: u32,
    package_fingerprints: impl IntoIterator<Item = PackageFingerprint>,
    entries: impl IntoIterator<Item = (AssetChange, Vec<u8>)>,
) -> Result<PatchManifest> {
    let mut writer = YapatchWriter::create(path, target_game, base_project_version)?;
    for fingerprint in package_fingerprints {
        writer.add_package_fingerprint(fingerprint);
    }
    for (change, payload) in entries {
        writer.add_change(change, &payload)?;
    }
    writer.finish()
}
