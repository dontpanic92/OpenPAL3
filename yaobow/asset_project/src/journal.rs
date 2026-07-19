//! Patch installation journal: an ordered, durable record of which
//! `.ybpatch` files have been (or are being) applied to a target
//! install.
//!
//! The journal exists so an interrupted install (crash, power loss)
//! can be detected and safely resumed or retried instead of silently
//! re-applying (or skipping) a patch. The write pattern mirrors
//! [`crate::manifest::ProjectManifest`]: the whole journal is a single
//! JSON document, persisted with [`crate::atomic::atomic_write`].

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::{atomic_write, read_file, unix_now};
use crate::error::{AssetProjectError, Result};
use crate::hash::ContentHash;

/// Highest `InstallationJournal::schema_version` this build knows how
/// to read.
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    /// Installation started but has not yet been confirmed complete.
    /// A journal loaded with a `Pending` entry at the tail indicates an
    /// interrupted install that needs to be retried or rolled back.
    Pending,
    Applied,
    Failed,
    RolledBack,
}

/// One record of a patch installation attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub patch_id: Uuid,
    /// Path to the `.ybpatch` file that was (or is being) installed,
    /// as given to [`InstallationJournal::begin`] — the journal does
    /// not resolve or canonicalize this.
    pub patch_path: std::path::PathBuf,
    /// Hash of the `.ybpatch` file's manifest bytes, so a later audit
    /// can confirm the exact patch content that was applied.
    pub manifest_hash: ContentHash,
    pub base_project_version: u32,
    pub status: InstallStatus,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    /// Package paths touched by this install, recorded once the patch
    /// is fully applied, so a rollback knows what to undo.
    pub changes_applied: Vec<String>,
    pub error: Option<String>,
}

/// Ordered, durable log of patch installation attempts against one
/// target install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationJournal {
    pub schema_version: u32,
    entries: Vec<JournalEntry>,
}

impl InstallationJournal {
    pub fn new() -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }

    /// Loads a journal from `path`, or returns a fresh empty journal if
    /// the file does not exist yet (a brand-new install has no journal
    /// on disk).
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        Self::load(path)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = read_file(path)?;
        let journal: Self =
            serde_json::from_slice(&bytes).map_err(|e| AssetProjectError::json(path, e))?;
        journal.check_version()?;
        Ok(journal)
    }

    fn check_version(&self) -> Result<()> {
        if self.schema_version > JOURNAL_SCHEMA_VERSION {
            return Err(AssetProjectError::UnsupportedManifestVersion {
                found: self.schema_version,
                supported: JOURNAL_SCHEMA_VERSION,
            });
        }
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| AssetProjectError::json(path, e))?;
        atomic_write(path, &bytes)
    }

    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    pub fn is_applied(&self, patch_id: Uuid) -> bool {
        self.entries
            .iter()
            .any(|e| e.patch_id == patch_id && e.status == InstallStatus::Applied)
    }

    fn find_mut(&mut self, patch_id: Uuid) -> Option<&mut JournalEntry> {
        self.entries
            .iter_mut()
            .rev()
            .find(|e| e.patch_id == patch_id)
    }

    /// Starts tracking an installation attempt, recording a `Pending`
    /// entry. Returns an error if this exact patch has already been
    /// successfully applied, so callers don't double-install by
    /// accident.
    pub fn begin(
        &mut self,
        patch_id: Uuid,
        patch_path: impl Into<std::path::PathBuf>,
        manifest_hash: ContentHash,
        base_project_version: u32,
    ) -> Result<()> {
        if self.is_applied(patch_id) {
            return Err(AssetProjectError::DuplicateJournalEntry(
                patch_id.to_string(),
            ));
        }

        self.entries.push(JournalEntry {
            patch_id,
            patch_path: patch_path.into(),
            manifest_hash,
            base_project_version,
            status: InstallStatus::Pending,
            started_at: unix_now(),
            finished_at: None,
            changes_applied: Vec::new(),
            error: None,
        });
        Ok(())
    }

    /// Marks a pending installation as successfully applied.
    pub fn complete(&mut self, patch_id: Uuid, changes_applied: Vec<String>) -> Result<()> {
        let now = unix_now();
        let entry = self
            .find_mut(patch_id)
            .ok_or_else(|| AssetProjectError::UnknownChange(patch_id.to_string()))?;
        entry.status = InstallStatus::Applied;
        entry.finished_at = Some(now);
        entry.changes_applied = changes_applied;
        entry.error = None;
        Ok(())
    }

    /// Marks a pending installation as failed, recording `error` for
    /// diagnostics.
    pub fn fail(&mut self, patch_id: Uuid, error: impl Into<String>) -> Result<()> {
        let now = unix_now();
        let entry = self
            .find_mut(patch_id)
            .ok_or_else(|| AssetProjectError::UnknownChange(patch_id.to_string()))?;
        entry.status = InstallStatus::Failed;
        entry.finished_at = Some(now);
        entry.error = Some(error.into());
        Ok(())
    }

    /// Marks a previously applied (or failed) installation as rolled
    /// back.
    pub fn roll_back(&mut self, patch_id: Uuid) -> Result<()> {
        let now = unix_now();
        let entry = self
            .find_mut(patch_id)
            .ok_or_else(|| AssetProjectError::UnknownChange(patch_id.to_string()))?;
        entry.status = InstallStatus::RolledBack;
        entry.finished_at = Some(now);
        Ok(())
    }

    /// Entries left in `Pending` state — i.e. installs that never
    /// reached a terminal status, most likely because the process
    /// crashed mid-install.
    pub fn pending_entries(&self) -> impl Iterator<Item = &JournalEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == InstallStatus::Pending)
    }
}

impl Default for InstallationJournal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!("{}-{}-{}", name, std::process::id(), unix_now()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn begin_then_complete_marks_applied() {
        let mut journal = InstallationJournal::new();
        let patch_id = Uuid::new_v4();
        let hash = ContentHash::of(b"manifest bytes");

        journal.begin(patch_id, "patch.ybpatch", hash, 1).unwrap();
        assert!(!journal.is_applied(patch_id));
        assert_eq!(journal.pending_entries().count(), 1);

        journal
            .complete(patch_id, vec!["a.txt".to_string()])
            .unwrap();
        assert!(journal.is_applied(patch_id));
        assert_eq!(journal.pending_entries().count(), 0);
        assert_eq!(journal.entries()[0].changes_applied, vec!["a.txt"]);
    }

    #[test]
    fn begin_twice_for_an_applied_patch_is_rejected() {
        let mut journal = InstallationJournal::new();
        let patch_id = Uuid::new_v4();
        let hash = ContentHash::of(b"manifest bytes");

        journal.begin(patch_id, "patch.ybpatch", hash, 1).unwrap();
        journal.complete(patch_id, vec![]).unwrap();

        let err = journal
            .begin(patch_id, "patch.ybpatch", hash, 1)
            .unwrap_err();
        assert!(matches!(err, AssetProjectError::DuplicateJournalEntry(_)));
    }

    #[test]
    fn fail_then_roll_back() {
        let mut journal = InstallationJournal::new();
        let patch_id = Uuid::new_v4();
        let hash = ContentHash::of(b"manifest bytes");

        journal.begin(patch_id, "patch.ybpatch", hash, 1).unwrap();
        journal.fail(patch_id, "disk full").unwrap();
        assert_eq!(journal.entries()[0].status, InstallStatus::Failed);
        assert_eq!(journal.entries()[0].error.as_deref(), Some("disk full"));

        journal.roll_back(patch_id).unwrap();
        assert_eq!(journal.entries()[0].status, InstallStatus::RolledBack);
    }

    #[test]
    fn save_and_load_or_default_round_trip() {
        let dir = scratch_dir("journal");
        let path = dir.join("journal.json");

        let missing = InstallationJournal::load_or_default(&path).unwrap();
        assert!(missing.entries().is_empty());

        let mut journal = InstallationJournal::new();
        let patch_id = Uuid::new_v4();
        journal
            .begin(patch_id, "patch.ybpatch", ContentHash::of(b"x"), 1)
            .unwrap();
        journal.complete(patch_id, vec!["a.txt".into()]).unwrap();
        journal.save(&path).unwrap();

        let loaded = InstallationJournal::load_or_default(&path).unwrap();
        assert_eq!(loaded.entries().len(), 1);
        assert!(loaded.is_applied(patch_id));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn operating_on_unknown_patch_id_is_an_error() {
        let mut journal = InstallationJournal::new();
        let err = journal.complete(Uuid::new_v4(), vec![]).unwrap_err();
        assert!(matches!(err, AssetProjectError::UnknownChange(_)));
    }

    #[test]
    fn retry_updates_the_latest_attempt() {
        let mut journal = InstallationJournal::new();
        let patch_id = Uuid::new_v4();
        let hash = ContentHash::of(b"manifest bytes");

        journal.begin(patch_id, "patch.ybpatch", hash, 1).unwrap();
        journal.fail(patch_id, "first attempt failed").unwrap();
        journal.begin(patch_id, "patch.ybpatch", hash, 1).unwrap();
        journal.complete(patch_id, vec!["a.txt".into()]).unwrap();

        assert_eq!(journal.entries()[0].status, InstallStatus::Failed);
        assert_eq!(journal.entries()[1].status, InstallStatus::Applied);
        assert_eq!(journal.pending_entries().count(), 0);
    }
}
