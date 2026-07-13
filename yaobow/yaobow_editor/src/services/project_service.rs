//! `IProjectService` Rust implementation: authoring-side asset project
//! lifecycle (create/open/save/close), tracked-change enumeration and
//! mutation, `.yapatch` publishing, and — via
//! `services::project_overlay` — keeping the editor's preview overlay
//! and `IResourceManager` category index in sync with the active
//! project.
//!
//! Project directory layout (a convention owned entirely by this
//! module, not by `asset_project`):
//!   `<project_dir>/project.json`  — `asset_project::ProjectManifest`
//!   `<project_dir>/payloads/`     — `asset_project::PayloadStore` root

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use asset_project::{
    AssetChange, AssetChangeKey, AssetChangeKind, AssetSource, ContentHash, ConversionMetadata,
    PackageFingerprint, PackagePath, PayloadStore, ProjectManifest, TargetPackage,
};
use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};
use packfs::AssetCatalog;

use crate::comdef::editor_services::{IPreviewerHub, IProjectService, IProjectServiceImpl};
use shared::GameType;

use super::project_overlay::SharedOverlayIndex;

const MANIFEST_FILE_NAME: &str = "project.json";
const PAYLOADS_DIR_NAME: &str = "payloads";

struct ActiveProject {
    manifest: ProjectManifest,
    project_dir: PathBuf,
    payload_store: PayloadStore,
}

pub struct ProjectServiceInner {
    game_type: GameType,
    base_asset_root: PathBuf,
    vfs: Rc<MiniFs>,
    catalog: Rc<AssetCatalog>,
    overlay: SharedOverlayIndex,
    previewers: ComRc<IPreviewerHub>,

    state: RefCell<Option<ActiveProject>>,
    dirty: Cell<bool>,
    last_error: RefCell<String>,
    last_string: RefCell<String>,
}

/// Cheaply-`Clone`-able Rust handle to a [`ProjectServiceInner`].
///
/// `IProjectService`'s p7/IDL surface only ever hands scripts an opaque
/// `ComRc<IProjectService>` (crosscom's CCW has no downcast-to-concrete
/// path), but `services::import_service::ImportService` needs a plain
/// Rust handle so it can call the richer, bytes-in-memory
/// [`ProjectServiceInner::stage_payload_bytes`] directly instead of
/// round-tripping converted bytes through a temp file and
/// `IProjectService::stage_payload_file`. [`ProjectService::create`]
/// hands out both this handle and the `ComRc` it wraps, from the same
/// underlying `Rc`, so scripts and other editor services always see a
/// single, consistent active-project state.
///
/// `Deref`s to [`ProjectServiceInner`] so every existing method body
/// below (written against `self.state`/`self.catalog`/etc., with
/// `self: &ProjectService`) keeps compiling unchanged — Rust resolves
/// field access through a `Deref` chain automatically.
#[derive(Clone)]
pub struct ProjectService(Rc<ProjectServiceInner>);

impl std::ops::Deref for ProjectService {
    type Target = ProjectServiceInner;

    fn deref(&self) -> &ProjectServiceInner {
        &self.0
    }
}

ComObject_ProjectService!(super::ProjectService);

impl ProjectService {
    /// Constructs both the plain Rust handle (for
    /// `services::import_service::ImportService` and any other
    /// in-process editor service that needs direct staging access) and
    /// the `ComRc<IProjectService>` p7/IDL surface, sharing the same
    /// underlying state.
    pub fn create(
        game_type: GameType,
        base_asset_root: PathBuf,
        vfs: Rc<MiniFs>,
        catalog: Rc<AssetCatalog>,
        overlay: SharedOverlayIndex,
        previewers: ComRc<IPreviewerHub>,
    ) -> (ProjectService, ComRc<IProjectService>) {
        let handle = ProjectService(Rc::new(ProjectServiceInner {
            game_type,
            base_asset_root,
            vfs,
            catalog,
            overlay,
            previewers,
            state: RefCell::new(None),
            dirty: Cell::new(false),
            last_error: RefCell::new(String::new()),
            last_string: RefCell::new(String::new()),
        }));
        let com_rc = ComRc::from_object(handle.clone());
        (handle, com_rc)
    }
}

impl ProjectServiceInner {
    fn set_last(&self, s: String) -> &str {
        *self.last_string.borrow_mut() = s;
        // SAFETY: see ConfigService::get_asset_path — single-threaded
        // script/UI path; codegen copies the &str into a CString
        // immediately on return, before this instance could be asked
        // to produce another `&str`.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn fail(&self, msg: impl Into<String>) -> bool {
        *self.last_error.borrow_mut() = msg.into();
        false
    }

    fn ok(&self) -> bool {
        self.last_error.borrow_mut().clear();
        true
    }

    fn manifest_path(project_dir: &Path) -> PathBuf {
        project_dir.join(MANIFEST_FILE_NAME)
    }

    fn payloads_path(project_dir: &Path) -> PathBuf {
        project_dir.join(PAYLOADS_DIR_NAME)
    }

    /// Resolves a `(target_package, package_internal_path)` pair to the
    /// absolute vfs path it lives (or would live) at, using the
    /// catalog recorded when the game's base VFS was mounted. Returns
    /// `None` if `target_package` isn't a physically-mounted package
    /// (e.g. a package the project itself would introduce) — such
    /// changes stay fully trackable/publishable, they just can't be
    /// live-previewed.
    pub(crate) fn resolve_vfs_path(&self, target_package: &TargetPackage) -> Option<PathBuf> {
        let wanted = target_package.as_str().replace('\\', "/");
        self.catalog.mounts().iter().find_map(|mount| {
            let phys = mount
                .physical_relative_path
                .to_string_lossy()
                .replace('\\', "/");
            (phys.eq_ignore_ascii_case(&wanted)).then(|| mount.vfs_mount_point.clone())
        })
    }

    pub(crate) fn vfs_path_for_parts(
        &self,
        target_package: &TargetPackage,
        internal_path: &PackagePath,
    ) -> Option<PathBuf> {
        self.resolve_vfs_path(target_package)
            .map(|mount_point| mount_point.join(internal_path.as_str()))
    }

    fn vfs_path_for(&self, change: &AssetChange) -> Option<PathBuf> {
        self.vfs_path_for_parts(&change.target_package, &change.package_internal_path)
    }

    /// Rebuilds the shared overlay index from the active project's
    /// tracked changes (or clears it if no project is active), then
    /// invalidates `IResourceManager`'s cached category index so the
    /// resources panel picks up any new/changed paths. Called
    /// automatically by every mutating method; also exposed to script.
    fn rebuild_overlay_index_impl(&self) {
        let state = self.state.borrow();
        match state.as_ref() {
            Some(active) => {
                let mut entries: HashMap<PathBuf, ContentHash> = HashMap::new();
                for change in active.manifest.changes() {
                    if let Some(path) = self.vfs_path_for(change) {
                        entries.insert(path, change.payload.content_hash);
                    }
                }
                self.overlay
                    .borrow_mut()
                    .reset(Some(active.payload_store_clone()), entries);
            }
            None => {
                self.overlay.borrow_mut().reset(None, HashMap::new());
            }
        }
        drop(state);
        self.previewers.resources().invalidate();
    }

    fn change_at(&self, index: i32) -> Option<AssetChange> {
        if index < 0 {
            return None;
        }
        let state = self.state.borrow();
        let active = state.as_ref()?;
        active.manifest.changes().nth(index as usize).cloned()
    }

    fn base_vfs_hash(&self, vfs_path: &Path) -> Option<ContentHash> {
        use std::io::Read;
        let mut file = self.vfs.open(vfs_path).ok()?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).ok()?;
        Some(ContentHash::of(&buf))
    }

    /// Rust-only counterpart of `IProjectService::stage_payload_file`
    /// that stages already-in-memory bytes instead of reading them from
    /// a temp file — the entry point
    /// `services::import_service::ImportService` uses so a converted
    /// glTF import's bytes are staged directly, without an extra
    /// disk round-trip. `stage_payload_file` itself now just reads the
    /// source file and delegates here.
    ///
    /// Determines `Add` vs. `Replace` (and captures the pre-existing
    /// `base_entry_hash` exactly once, the first time this
    /// `(target_package, internal_path)` key is staged) the same way
    /// `stage_payload_file` always has: see `resolve_vfs_path`'s doc
    /// comment and `services::project_overlay`'s module docs for why a
    /// `self.vfs.open` read still faithfully reflects the *base*
    /// package at this point.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_payload_bytes(
        &self,
        target_package: &str,
        internal_path: &str,
        bytes: &[u8],
        source_path: &str,
        tool: &str,
        tool_version: &str,
    ) -> Result<(), String> {
        if self.state.borrow().is_none() {
            return Err("no active project".to_string());
        }

        let target_package = TargetPackage::new(target_package)
            .map_err(|e| format!("invalid target_package: {e}"))?;
        let internal_path =
            PackagePath::new(internal_path).map_err(|e| format!("invalid internal_path: {e}"))?;

        let key = AssetChangeKey::new(target_package.clone(), internal_path.clone());
        let existing = self
            .state
            .borrow()
            .as_ref()
            .and_then(|a| a.manifest.get_change(&key).cloned());

        let (kind, base_entry_hash) = match &existing {
            Some(prev) => (prev.kind, prev.base_entry_hash),
            None => match self.vfs_path_for_parts(&target_package, &internal_path) {
                Some(vfs_path) => match self.base_vfs_hash(&vfs_path) {
                    Some(hash) => (AssetChangeKind::Replace, Some(hash)),
                    None => (AssetChangeKind::Add, None),
                },
                None => (AssetChangeKind::Add, None),
            },
        };

        let conversion = if tool.is_empty() && tool_version.is_empty() {
            None
        } else {
            Some(ConversionMetadata {
                tool: tool.to_string(),
                tool_version: tool_version.to_string(),
                params: BTreeMap::new(),
                converted_at: asset_project::atomic::unix_now(),
            })
        };
        let source = Some(AssetSource {
            original_path: PathBuf::from(source_path),
            source_hash: Some(ContentHash::of(bytes)),
        });

        let change = AssetChange::from_payload(
            kind,
            target_package,
            internal_path,
            bytes,
            base_entry_hash,
            source,
            conversion,
        );

        {
            let mut state = self.state.borrow_mut();
            let Some(active) = state.as_mut() else {
                return Err("no active project".to_string());
            };
            active
                .payload_store
                .put(bytes)
                .map_err(|e| format!("failed to store payload: {e}"))?;
            active.manifest.upsert_change(change);
        }
        self.dirty.set(true);
        self.rebuild_overlay_index_impl();
        Ok(())
    }
}

impl ActiveProject {
    // `PayloadStore` has no `Clone`; project-authored paths always
    // resolve back to the same on-disk root, so a fresh handle over
    // the same root is equivalent to a clone for our purposes.
    fn payload_store_clone(&self) -> PayloadStore {
        PayloadStore::new(self.payload_store.root())
    }
}

fn normalize_path_str(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn paths_match(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        // Fall back to a normalized string comparison when either side
        // can't be canonicalized (e.g. it doesn't exist on this
        // machine, or in unit tests using scratch directories that are
        // relative).
        _ => normalize_path_str(a) == normalize_path_str(b),
    }
}

impl IProjectServiceImpl for ProjectService {
    fn create_project(&self, project_dir: &str) -> bool {
        if self.dirty.get() {
            return self.fail("active project has unsaved changes; save or discard them first");
        }
        if project_dir.is_empty() {
            return self.fail("project_dir must not be empty");
        }
        let project_dir = PathBuf::from(project_dir);
        let manifest_path = ProjectServiceInner::manifest_path(&project_dir);
        if manifest_path.exists() {
            return self.fail(format!(
                "a project already exists at {}",
                manifest_path.display()
            ));
        }
        let name = project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        if let Err(e) = std::fs::create_dir_all(&project_dir) {
            return self.fail(format!("failed to create {}: {e}", project_dir.display()));
        }

        let manifest = ProjectManifest::new(
            name,
            self.game_type.config_key().to_string(),
            self.base_asset_root.clone(),
        );
        if let Err(e) = manifest.save(&manifest_path) {
            return self.fail(format!("failed to save project manifest: {e}"));
        }

        let payload_store = PayloadStore::new(ProjectServiceInner::payloads_path(&project_dir));
        if let Err(e) = std::fs::create_dir_all(payload_store.root()) {
            return self.fail(format!("failed to create payload store: {e}"));
        }

        *self.state.borrow_mut() = Some(ActiveProject {
            manifest,
            project_dir,
            payload_store,
        });
        self.dirty.set(false);
        self.rebuild_overlay_index_impl();
        self.ok()
    }

    fn open_project(&self, project_path: &str) -> bool {
        if self.dirty.get() {
            return self.fail("active project has unsaved changes; save or discard them first");
        }
        if project_path.is_empty() {
            return self.fail("project_path must not be empty");
        }
        let path = PathBuf::from(project_path);
        let (manifest_path, project_dir) = if path.is_dir() {
            (ProjectServiceInner::manifest_path(&path), path.clone())
        } else {
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (path.clone(), dir)
        };

        let manifest = match ProjectManifest::load(&manifest_path) {
            Ok(m) => m,
            Err(e) => {
                return self.fail(format!(
                    "failed to load project manifest {}: {e}",
                    manifest_path.display()
                ));
            }
        };

        let expected_game = self.game_type.config_key();
        if !manifest.target_game.eq_ignore_ascii_case(expected_game) {
            return self.fail(format!(
                "project targets game {:?} but the editor currently has {:?} open",
                manifest.target_game, expected_game
            ));
        }
        if !paths_match(&manifest.base_asset_root, &self.base_asset_root) {
            return self.fail(format!(
                "project base_asset_root {:?} does not match the currently configured asset path {:?}",
                manifest.base_asset_root, self.base_asset_root
            ));
        }

        let payload_store = PayloadStore::new(ProjectServiceInner::payloads_path(&project_dir));
        *self.state.borrow_mut() = Some(ActiveProject {
            manifest,
            project_dir,
            payload_store,
        });
        self.dirty.set(false);
        self.rebuild_overlay_index_impl();
        self.ok()
    }

    fn save_project(&self) -> bool {
        let state = self.state.borrow();
        let Some(active) = state.as_ref() else {
            drop(state);
            return self.fail("no active project");
        };
        let result = active
            .manifest
            .save(ProjectServiceInner::manifest_path(&active.project_dir));
        drop(state);
        match result {
            Ok(()) => {
                self.dirty.set(false);
                self.ok()
            }
            Err(e) => self.fail(format!("failed to save project: {e}")),
        }
    }

    fn close_project(&self) -> bool {
        if self.state.borrow().is_none() {
            return self.fail("no active project");
        }
        if self.dirty.get() {
            return self.fail("project has unsaved changes; save or explicitly discard them first");
        }
        *self.state.borrow_mut() = None;
        self.dirty.set(false);
        self.rebuild_overlay_index_impl();
        self.ok()
    }

    fn discard_and_close_project(&self) -> bool {
        if self.state.borrow().is_none() {
            return self.fail("no active project");
        }
        *self.state.borrow_mut() = None;
        self.dirty.set(false);
        self.rebuild_overlay_index_impl();
        self.ok()
    }

    fn has_active_project(&self) -> bool {
        self.state.borrow().is_some()
    }

    fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    fn active_project_name(&self) -> &str {
        let s = self
            .state
            .borrow()
            .as_ref()
            .map(|a| a.manifest.name.clone())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn active_project_dir(&self) -> &str {
        let s = self
            .state
            .borrow()
            .as_ref()
            .map(|a| a.project_dir.to_string_lossy().to_string())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn active_project_target_game(&self) -> &str {
        let s = self
            .state
            .borrow()
            .as_ref()
            .map(|a| a.manifest.target_game.clone())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn active_project_base_asset_root(&self) -> &str {
        let s = self
            .state
            .borrow()
            .as_ref()
            .map(|a| a.manifest.base_asset_root.to_string_lossy().to_string())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_count(&self) -> i32 {
        self.state
            .borrow()
            .as_ref()
            .map(|a| a.manifest.len() as i32)
            .unwrap_or(0)
    }

    fn change_kind(&self, index: i32) -> i32 {
        match self.change_at(index).map(|c| c.kind) {
            Some(AssetChangeKind::Add) => 0,
            Some(AssetChangeKind::Replace) => 1,
            None => -1,
        }
    }

    fn change_target_package(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .map(|c| c.target_package.as_str().to_string())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_internal_path(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .map(|c| c.package_internal_path.as_str().to_string())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_content_hash(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .map(|c| c.payload.content_hash.to_hex())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_payload_size(&self, index: i32) -> i32 {
        self.change_at(index)
            .map(|c| c.payload.size.min(i32::MAX as u64) as i32)
            .unwrap_or(0)
    }

    fn change_base_hash(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .and_then(|c| c.base_entry_hash)
            .map(|h| h.to_hex())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_source_path(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .and_then(|c| c.source)
            .map(|s| s.original_path.to_string_lossy().to_string())
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_conversion_tool(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .and_then(|c| c.conversion)
            .map(|c| c.tool)
            .unwrap_or_default();
        self.set_last(s)
    }

    fn change_conversion_tool_version(&self, index: i32) -> &str {
        let s = self
            .change_at(index)
            .and_then(|c| c.conversion)
            .map(|c| c.tool_version)
            .unwrap_or_default();
        self.set_last(s)
    }

    fn remove_change(&self, index: i32) -> bool {
        let Some(change) = self.change_at(index) else {
            return self.fail(format!("no change at index {index}"));
        };
        {
            let mut state = self.state.borrow_mut();
            let Some(active) = state.as_mut() else {
                return self.fail("no active project");
            };
            active.manifest.remove_change(&change.key());
        }
        self.dirty.set(true);
        self.rebuild_overlay_index_impl();
        self.ok()
    }

    fn revert_change(&self, index: i32) -> bool {
        // Same effect as `remove_change` today: there is no separate
        // "live" filesystem write to roll back, since staged changes
        // only ever exist in the manifest + payload store + preview
        // overlay. Kept as a distinct entry point so the "Project
        // Changes" panel can offer a semantically-labelled "revert to
        // base" action independent of "remove from list".
        self.remove_change(index)
    }

    fn stage_payload_file(
        &self,
        target_package: &str,
        internal_path: &str,
        source_file_path: &str,
        tool: &str,
        tool_version: &str,
    ) -> bool {
        let bytes = match std::fs::read(source_file_path) {
            Ok(b) => b,
            Err(e) => {
                return self.fail(format!("failed to read {source_file_path}: {e}"));
            }
        };
        match self.stage_payload_bytes(
            target_package,
            internal_path,
            &bytes,
            source_file_path,
            tool,
            tool_version,
        ) {
            Ok(()) => self.ok(),
            Err(e) => self.fail(e),
        }
    }

    fn rebuild_overlay_index(&self) {
        self.rebuild_overlay_index_impl();
    }

    fn publish_patch(&self, output_path: &str) -> bool {
        if output_path.is_empty() {
            return self.fail("output_path must not be empty");
        }

        // Snapshot everything we need as owned data up front, then
        // drop the borrow immediately — avoids holding a `Ref` across
        // the fallible payload-read loop below.
        let (changes, target_game, version, payload_store) = {
            let state = self.state.borrow();
            let Some(active) = state.as_ref() else {
                return self.fail("no active project");
            };
            if active.manifest.is_empty() {
                return self.fail("project has no tracked changes to publish");
            }
            (
                active.manifest.changes().cloned().collect::<Vec<_>>(),
                active.manifest.target_game.clone(),
                active.manifest.version,
                active.payload_store_clone(),
            )
        };

        let mut fingerprints: Vec<PackageFingerprint> = Vec::new();
        let mut seen_packages: std::collections::HashSet<String> = Default::default();
        let mut entries: Vec<(AssetChange, Vec<u8>)> = Vec::new();

        for change in &changes {
            if seen_packages.insert(change.target_package.as_str().to_string()) {
                if let Some(mount) = self.catalog.mounts().iter().find(|m| {
                    m.physical_relative_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .eq_ignore_ascii_case(&change.target_package.as_str().replace('\\', "/"))
                }) {
                    let physical_path = self.catalog.physical_path(mount);
                    match std::fs::read(&physical_path) {
                        Ok(bytes) => fingerprints.push(PackageFingerprint {
                            target_package: change.target_package.clone(),
                            base_hash: ContentHash::of(&bytes),
                        }),
                        Err(e) => {
                            log::warn!(
                                "publish_patch: failed to fingerprint {}: {e}",
                                physical_path.display()
                            );
                        }
                    }
                }
            }

            let bytes = match payload_store.get(change.payload.content_hash) {
                Ok(b) => b,
                Err(e) => {
                    return self.fail(format!("failed to read staged payload: {e}"));
                }
            };
            entries.push((change.clone(), bytes));
        }

        let result =
            asset_project::publish(output_path, target_game, version, fingerprints, entries);

        match result {
            Ok(_) => self.ok(),
            Err(e) => self.fail(format!("failed to publish patch: {e}")),
        }
    }

    fn last_error(&self) -> &str {
        let s = self.last_error.borrow().clone();
        self.set_last(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comdef::editor_services::{
        IAudioHandle, IImageHandle, IModelHandle, IPreviewerHubImpl, ISceneHandle,
    };
    use crate::comdef::services::IUiLayoutHandle;
    use crate::services::ResourceManager;

    struct StubPreviewerHub;
    ComObject_PreviewerHub!(crate::services::project_service::tests::StubPreviewerHub);
    impl IPreviewerHubImpl for StubPreviewerHub {
        fn classify(&self, _vfs_path: &str) -> i32 {
            0
        }
        fn open_text(&self, _vfs_path: &str) -> &str {
            ""
        }
        fn dump_structured(&self, _vfs_path: &str) -> &str {
            ""
        }
        fn open_image(&self, _vfs_path: &str) -> Option<ComRc<IImageHandle>> {
            None
        }
        fn open_audio(&self, _vfs_path: &str) -> Option<ComRc<IAudioHandle>> {
            None
        }
        fn open_video(
            &self,
            _vfs_path: &str,
        ) -> Option<ComRc<crate::comdef::services::IVideoHandle>> {
            None
        }
        fn open_model(&self, _vfs_path: &str) -> Option<ComRc<IModelHandle>> {
            None
        }
        fn open_scene(&self, _vfs_path: &str) -> Option<ComRc<ISceneHandle>> {
            None
        }
        fn open_ui_layout(&self, _vfs_path: &str) -> Option<ComRc<IUiLayoutHandle>> {
            None
        }
        fn resources(&self) -> ComRc<crate::comdef::editor_services::IResourceManager> {
            ResourceManager::create(Rc::new(MiniFs::new(false)))
        }
    }

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

    fn make_service(base_asset_root: PathBuf) -> ComRc<IProjectService> {
        make_service_with_handle(base_asset_root).1
    }

    /// Like `make_service`, but also returns the plain Rust
    /// `ProjectService` handle (e.g. for tests exercising
    /// `stage_payload_bytes` directly, the same handle
    /// `services::import_service::ImportService` is given).
    pub(crate) fn make_service_with_handle(
        base_asset_root: PathBuf,
    ) -> (ProjectService, ComRc<IProjectService>) {
        let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub);
        ProjectService::create(
            GameType::PAL3,
            base_asset_root,
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            super::super::project_overlay::new_shared_overlay_index(),
            previewers,
        )
    }

    #[test]
    fn create_project_creates_manifest_and_activates() {
        let root = scratch_dir("create-basic");
        let base_root = root.join("base_assets");
        let project_dir = root.join("myproj");
        let svc = make_service(base_root.clone());

        assert!(svc.create_project(project_dir.to_str().unwrap()));
        assert!(svc.has_active_project());
        assert_eq!(svc.active_project_name(), "myproj");
        assert_eq!(PathBuf::from(svc.active_project_dir()), project_dir.clone());
        assert_eq!(svc.active_project_target_game(), "pal3");
        assert_eq!(
            PathBuf::from(svc.active_project_base_asset_root()),
            base_root
        );
        assert!(ProjectServiceInner::manifest_path(&project_dir).exists());
    }

    #[test]
    fn create_project_fails_when_already_exists() {
        let root = scratch_dir("create-duplicate");
        let project_dir = root.join("proj");
        let svc = make_service(root.join("base"));

        assert!(svc.create_project(project_dir.to_str().unwrap()));
        assert!(svc.close_project());

        let svc2 = make_service(root.join("base"));
        assert!(!svc2.create_project(project_dir.to_str().unwrap()));
        assert!(!svc2.last_error().is_empty());
    }

    #[test]
    fn create_project_fails_on_empty_dir() {
        let svc = make_service(PathBuf::from("/base"));
        assert!(!svc.create_project(""));
        assert!(!svc.last_error().is_empty());
    }

    #[test]
    fn open_project_round_trips_and_validates_game_and_base_root() {
        let root = scratch_dir("open-validate");
        let base_root = root.join("base_assets");
        let project_dir = root.join("proj");

        let creator = make_service(base_root.clone());
        assert!(creator.create_project(project_dir.to_str().unwrap()));
        assert!(creator.close_project());

        // Matching game + base root: opens successfully.
        let opener = make_service(base_root.clone());
        assert!(opener.open_project(project_dir.to_str().unwrap()));
        assert!(opener.has_active_project());
        assert_eq!(opener.active_project_name(), "proj");

        // Mismatched base_asset_root: fails, leaves no active project.
        let wrong_root = make_service(root.join("other_base"));
        assert!(!wrong_root.open_project(project_dir.to_str().unwrap()));
        assert!(!wrong_root.has_active_project());
        assert!(!wrong_root.last_error().is_empty());
    }

    #[test]
    fn stage_remove_and_revert_change_round_trip() {
        let root = scratch_dir("stage-remove-revert");
        let project_dir = root.join("proj");
        let svc = make_service(root.join("base"));
        assert!(svc.create_project(project_dir.to_str().unwrap()));

        let source_file = root.join("source.txt");
        std::fs::write(&source_file, b"hello world").unwrap();

        assert!(svc.stage_payload_file(
            "scene/q01.cpk",
            "q01/q01.scn",
            source_file.to_str().unwrap(),
            "",
            "",
        ));
        assert_eq!(svc.change_count(), 1);
        // No catalog mount for "scene/q01.cpk" => treated as a new Add.
        assert_eq!(svc.change_kind(0), 0);
        assert_eq!(svc.change_target_package(0), "scene/q01.cpk");
        assert_eq!(svc.change_internal_path(0), "q01/q01.scn");
        assert_eq!(svc.change_payload_size(0), "hello world".len() as i32);
        assert!(!svc.change_content_hash(0).is_empty());

        assert!(svc.remove_change(0));
        assert_eq!(svc.change_count(), 0);

        // revert_change is remove_change's alias; re-stage then revert.
        assert!(svc.stage_payload_file(
            "scene/q01.cpk",
            "q01/q01.scn",
            source_file.to_str().unwrap(),
            "",
            "",
        ));
        assert_eq!(svc.change_count(), 1);
        assert!(svc.revert_change(0));
        assert_eq!(svc.change_count(), 0);
    }

    #[test]
    fn dirty_project_cannot_be_silently_replaced_or_closed() {
        let root = scratch_dir("dirty-lifecycle");
        let project_dir = root.join("proj");
        let replacement_dir = root.join("replacement");
        let svc = make_service(root.join("base"));
        assert!(svc.create_project(project_dir.to_str().unwrap()));

        let source_file = root.join("source.bin");
        std::fs::write(&source_file, b"unsaved").unwrap();
        assert!(svc.stage_payload_file(
            "scene/q01.cpk",
            "q01/q01.scn",
            source_file.to_str().unwrap(),
            "",
            "",
        ));
        assert!(svc.is_dirty());
        assert!(!svc.close_project());
        assert!(!svc.create_project(replacement_dir.to_str().unwrap()));
        assert!(svc.has_active_project());

        assert!(svc.save_project());
        assert!(!svc.is_dirty());
        assert!(svc.close_project());

        assert!(svc.open_project(project_dir.to_str().unwrap()));
        assert!(svc.remove_change(0));
        assert!(svc.is_dirty());
        assert!(svc.discard_and_close_project());
        assert!(!svc.has_active_project());
    }

    #[test]
    fn stage_payload_file_fails_without_active_project() {
        let svc = make_service(PathBuf::from("/base"));
        assert!(!svc.stage_payload_file("scene/q01.cpk", "q01/q01.scn", "/no/such/file", "", ""));
        assert!(!svc.last_error().is_empty());
    }

    #[test]
    fn publish_patch_round_trips_through_yapatch_reader() {
        let root = scratch_dir("publish-roundtrip");
        let project_dir = root.join("proj");
        let svc = make_service(root.join("base"));
        assert!(svc.create_project(project_dir.to_str().unwrap()));

        let source_file = root.join("source.bin");
        std::fs::write(&source_file, b"payload bytes").unwrap();
        assert!(svc.stage_payload_file(
            "scene/q01.cpk",
            "q01/q01.scn",
            source_file.to_str().unwrap(),
            "gltf_import",
            "0.1",
        ));

        let patch_path = root.join("out.yapatch");
        assert!(svc.publish_patch(patch_path.to_str().unwrap()));
        assert!(patch_path.exists());

        let mut reader = asset_project::YapatchReader::open(&patch_path).unwrap();
        assert_eq!(reader.manifest().target_game, "pal3");
        assert_eq!(reader.manifest().changes.len(), 1);
        let change = reader.manifest().changes[0].clone();
        assert_eq!(change.target_package.as_str(), "scene/q01.cpk");
        assert_eq!(change.package_internal_path.as_str(), "q01/q01.scn");
        let bytes = reader.read_payload(&change).unwrap();
        assert_eq!(bytes, b"payload bytes");
    }

    #[test]
    fn publish_patch_fails_with_no_active_project_or_no_changes() {
        let root = scratch_dir("publish-empty");
        let svc = make_service(root.join("base"));
        assert!(!svc.publish_patch(root.join("out.yapatch").to_str().unwrap()));

        let project_dir = root.join("proj");
        assert!(svc.create_project(project_dir.to_str().unwrap()));
        assert!(!svc.publish_patch(root.join("out2.yapatch").to_str().unwrap()));
    }
}
