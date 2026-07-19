//! Durable mod-manager state and managed `.ybpatch` library.

use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use asset_project::atomic::{atomic_write, read_file};
use asset_project::hash::ContentHash;
use asset_project::patch::{PatchManifest, YbpatchReader};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{PatcherError, Result};
use crate::transaction::PatchPaths;

pub const MANAGER_STATE_FILE_NAME: &str = "manager.json";
pub const LIBRARY_DIR_NAME: &str = "library";
pub const OPERATIONS_DIR_NAME: &str = "operations";
const MANAGER_STATE_SCHEMA_VERSION: u32 = 1;

pub(crate) struct RootLock {
    _file: fs::File,
}

impl RootLock {
    pub(crate) fn acquire(game_root: &Path) -> Result<Self> {
        let canonical_root =
            fs::canonicalize(game_root).map_err(|error| PatcherError::io(game_root, error))?;
        let mut identity = canonical_root.to_string_lossy().into_owned();
        if cfg!(windows) {
            identity.make_ascii_lowercase();
        }

        let lock_dir = std::env::temp_dir().join("yaobow-asset-patcher-locks");
        fs::create_dir_all(&lock_dir).map_err(|error| PatcherError::io(&lock_dir, error))?;
        let lock_path = lock_dir.join(format!("{}.lock", ContentHash::of(identity.as_bytes())));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .map_err(|error| PatcherError::io(&lock_path, error))?;

        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(PatcherError::Other(format!(
                    "another Yaobow mod-manager operation is already using {}",
                    canonical_root.display()
                )))
            }
            Err(error) => Err(PatcherError::io(&lock_path, error)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerState {
    pub schema_version: u32,
    #[serde(default)]
    pub applied_order: Vec<Uuid>,
    #[serde(default)]
    pub package_heads: BTreeMap<String, ContentHash>,
    #[serde(default)]
    pub source_names: BTreeMap<Uuid, String>,
}

impl Default for ManagerState {
    fn default() -> Self {
        Self {
            schema_version: MANAGER_STATE_SCHEMA_VERSION,
            applied_order: Vec::new(),
            package_heads: BTreeMap::new(),
            source_names: BTreeMap::new(),
        }
    }
}

impl ManagerState {
    pub fn load_or_default(game_root: &Path) -> Result<Self> {
        let path = manager_state_path(game_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = read_file(&path)?;
        let state: Self =
            serde_json::from_slice(&bytes).map_err(|error| PatcherError::json(&path, error))?;
        if state.schema_version > MANAGER_STATE_SCHEMA_VERSION {
            return Err(PatcherError::UnsupportedManagerStateVersion {
                found: state.schema_version,
                supported: MANAGER_STATE_SCHEMA_VERSION,
            });
        }
        Ok(state)
    }

    pub fn save(&self, game_root: &Path) -> Result<()> {
        let path = manager_state_path(game_root);
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| PatcherError::json(&path, error))?;
        atomic_write(&path, &bytes)?;
        Ok(())
    }

    pub fn is_applied(&self, patch_id: Uuid) -> bool {
        self.applied_order.contains(&patch_id)
    }

    pub fn mark_applied(&mut self, patch_id: Uuid) {
        if !self.is_applied(patch_id) {
            self.applied_order.push(patch_id);
        }
    }

    pub fn mark_uninstalled(&mut self, patch_id: Uuid) {
        self.applied_order.retain(|id| *id != patch_id);
    }

    pub fn package_head(&self, target_package: &str) -> Option<ContentHash> {
        self.package_heads
            .get(&normalize_package_name(target_package))
            .copied()
    }

    pub fn set_package_head(&mut self, target_package: &str, hash: ContentHash) {
        self.package_heads
            .insert(normalize_package_name(target_package), hash);
    }

    pub fn remove_package_head(&mut self, target_package: &str) {
        self.package_heads
            .remove(&normalize_package_name(target_package));
    }

    pub fn source_name(&self, patch_id: Uuid) -> Option<&str> {
        self.source_names.get(&patch_id).map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct ManagedPatch {
    pub path: PathBuf,
    pub file_hash: ContentHash,
    pub manifest: PatchManifest,
}

impl ManagedPatch {
    pub fn patch_id(&self) -> Uuid {
        self.manifest.patch_id
    }
}

pub(crate) struct PatchSnapshot {
    pub(crate) bytes: Arc<[u8]>,
    file_hash: ContentHash,
    pub(crate) manifest: PatchManifest,
    source_name: String,
}

pub(crate) fn read_patch_snapshot(source_path: &Path) -> Result<PatchSnapshot> {
    let bytes: Arc<[u8]> = fs::read(source_path)
        .map_err(|error| PatcherError::io(source_path, error))?
        .into();
    let file_hash = ContentHash::of(&bytes);
    let mut reader = YbpatchReader::from_bytes(bytes.clone())?;
    reader.verify_all()?;
    let manifest = reader.manifest().clone();
    let source_name = source_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{}.ybpatch", manifest.patch_id));
    Ok(PatchSnapshot {
        bytes,
        file_hash,
        manifest,
        source_name,
    })
}

pub(crate) fn import_snapshot(game_root: &Path, snapshot: &PatchSnapshot) -> Result<ManagedPatch> {
    let destination = managed_patch_path(game_root, snapshot.manifest.patch_id);
    if destination.exists() {
        let existing =
            fs::read(&destination).map_err(|error| PatcherError::io(&destination, error))?;
        if ContentHash::of(&existing) != snapshot.file_hash {
            return Err(PatcherError::ManagedPatchIdCollision {
                patch_id: snapshot.manifest.patch_id,
            });
        }
    } else {
        atomic_write(&destination, &snapshot.bytes)?;
    }

    let mut state = ManagerState::load_or_default(game_root)?;
    state
        .source_names
        .entry(snapshot.manifest.patch_id)
        .or_insert_with(|| snapshot.source_name.clone());
    state.save(game_root)?;

    load_managed_patch(game_root, snapshot.manifest.patch_id)
}

pub fn import_patch(
    game_root: &Path,
    source_path: &Path,
    expected_game: &str,
) -> Result<ManagedPatch> {
    let _root_lock = RootLock::acquire(game_root)?;
    let snapshot = read_patch_snapshot(source_path)?;
    if snapshot.manifest.target_game != expected_game {
        return Err(PatcherError::GameMismatch {
            patch_game: snapshot.manifest.target_game,
            root_game: expected_game.to_string(),
        });
    }
    import_snapshot(game_root, &snapshot)
}

pub fn load_managed_patch(game_root: &Path, patch_id: Uuid) -> Result<ManagedPatch> {
    let path = managed_patch_path(game_root, patch_id);
    if !path.is_file() {
        return Err(PatcherError::ManagedPatchNotFound(patch_id));
    }
    let bytes = fs::read(&path).map_err(|error| PatcherError::io(&path, error))?;
    let mut reader = YbpatchReader::open(&path)?;
    reader.verify_all()?;
    let manifest = reader.manifest().clone();
    if manifest.patch_id != patch_id {
        return Err(PatcherError::Other(format!(
            "managed patch filename identifies {patch_id}, but its manifest identifies {}",
            manifest.patch_id
        )));
    }
    Ok(ManagedPatch {
        path,
        file_hash: ContentHash::of(&bytes),
        manifest,
    })
}

pub fn list_managed_patches(game_root: &Path) -> Result<Vec<ManagedPatch>> {
    let library = library_dir(game_root);
    if !library.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(&library).map_err(|error| PatcherError::io(&library, error))? {
        let entry = entry.map_err(|error| PatcherError::io(&library, error))?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "ybpatch")
        {
            paths.push(path);
        }
    }
    paths.sort();

    let mut patches = Vec::with_capacity(paths.len());
    for path in paths {
        let mut reader = YbpatchReader::open(&path)?;
        reader.verify_all()?;
        let manifest = reader.manifest().clone();
        let expected_path = managed_patch_path(game_root, manifest.patch_id);
        if path != expected_path {
            return Err(PatcherError::Other(format!(
                "managed patch {} is not stored at its canonical path {}",
                path.display(),
                expected_path.display()
            )));
        }
        let bytes = fs::read(&path).map_err(|error| PatcherError::io(&path, error))?;
        patches.push(ManagedPatch {
            path,
            file_hash: ContentHash::of(&bytes),
            manifest,
        });
    }
    Ok(patches)
}

pub fn load_applied_patches(game_root: &Path, state: &ManagerState) -> Result<Vec<ManagedPatch>> {
    state
        .applied_order
        .iter()
        .map(|patch_id| load_managed_patch(game_root, *patch_id))
        .collect()
}

pub fn manager_state_path(game_root: &Path) -> PathBuf {
    PatchPaths::for_root(game_root)
        .patch_state_dir
        .join(MANAGER_STATE_FILE_NAME)
}

pub fn library_dir(game_root: &Path) -> PathBuf {
    PatchPaths::for_root(game_root)
        .patch_state_dir
        .join(LIBRARY_DIR_NAME)
}

pub fn managed_patch_path(game_root: &Path, patch_id: Uuid) -> PathBuf {
    library_dir(game_root).join(format!("{patch_id}.ybpatch"))
}

pub fn operations_dir(game_root: &Path) -> PathBuf {
    PatchPaths::for_root(game_root)
        .patch_state_dir
        .join(OPERATIONS_DIR_NAME)
}

pub fn operation_dir(game_root: &Path, operation_id: Uuid) -> PathBuf {
    operations_dir(game_root).join(operation_id.to_string())
}

pub fn normalize_package_name(target_package: &str) -> String {
    target_package.replace('\\', "/").to_lowercase()
}

pub fn normalize_internal_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{FixtureChange, build_fixture_ybpatch};

    #[test]
    fn root_lock_rejects_a_second_process_slot() {
        let root = crate::test_scratch::dir("manager-root-lock");
        let _first = RootLock::acquire(&root).unwrap();
        assert!(matches!(
            RootLock::acquire(&root),
            Err(PatcherError::Other(message)) if message.contains("already using")
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn state_round_trips_and_normalizes_package_heads() {
        let root = crate::test_scratch::dir("manager-state");
        let patch_id = Uuid::new_v4();
        let mut state = ManagerState::default();
        state.mark_applied(patch_id);
        state.set_package_head(r"Basedata\Basedata.cpk", ContentHash::of(b"head"));
        state.save(&root).unwrap();

        let loaded = ManagerState::load_or_default(&root).unwrap();
        assert!(loaded.is_applied(patch_id));
        assert_eq!(
            loaded.package_head("basedata/basedata.cpk"),
            Some(ContentHash::of(b"head"))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_is_idempotent_and_rejects_same_id_with_different_content() {
        let root = crate::test_scratch::dir("manager-import");
        let source = root.join("source.ybpatch");
        let manifest = build_fixture_ybpatch(
            &source,
            "pal3",
            1,
            &[],
            &[FixtureChange::add("scene.cpk", "new.txt", b"payload")],
        );

        let first = import_patch(&root, &source, "pal3").unwrap();
        let second = import_patch(&root, &source, "pal3").unwrap();
        assert_eq!(first.patch_id(), manifest.patch_id);
        assert_eq!(first.file_hash, second.file_hash);
        assert_eq!(list_managed_patches(&root).unwrap().len(), 1);

        fs::write(
            managed_patch_path(&root, manifest.patch_id),
            b"different bytes",
        )
        .unwrap();
        assert!(matches!(
            import_patch(&root, &source, "pal3"),
            Err(PatcherError::ManagedPatchIdCollision { .. })
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_rejects_wrong_game_without_copying() {
        let root = crate::test_scratch::dir("manager-wrong-game");
        let source = root.join("source.ybpatch");
        let manifest = build_fixture_ybpatch(&source, "pal4", 1, &[], &[]);

        assert!(matches!(
            import_patch(&root, &source, "pal3"),
            Err(PatcherError::GameMismatch { .. })
        ));
        assert!(!managed_patch_path(&root, manifest.patch_id).exists());

        let _ = fs::remove_dir_all(root);
    }
}
