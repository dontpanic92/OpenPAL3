//! `IImportService` Rust implementation: the unified glTF (`.glb`/
//! `.gltf`) import wizard. Wraps
//! `shared::importers::convert_gltf_to_bytes` with the bookkeeping a
//! script-driven wizard needs on top of it:
//!
//!   - Replace-mode target resolution through the `packfs` catalog
//!     (mirrors `ProjectServiceInner::resolve_vfs_path`), reading the
//!     resolved vfs path's current bytes as the conversion template.
//!   - Add-mode target-package/-directory selection (from a
//!     `.cpk`-filtered catalog listing) plus a derived (never
//!     free-typed — the p7 UI has no text-input widget) internal path
//!     and collision check.
//!   - A conversion cache so `convert()`/`run()` only ever call
//!     `convert_gltf_to_bytes` once per distinct set of inputs; any
//!     setter invalidates it.
//!   - Independent "save converted bytes to a file" /"stage into the
//!     active project" outputs, routing the exact same converted bytes
//!     to both when both are enabled, with an explicit
//!     not-run/success/partial/failed `status()` (not just a single
//!     success boolean).
//!   - Staging goes through `ProjectServiceInner::stage_payload_bytes`
//!     directly (the plain-Rust handle
//!     `services::project_service::ProjectService`), never round-
//!     tripping the converted bytes through a temp file.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};
use packfs::{AssetCatalog, PackageType};

use shared::importers::{
    CvdOptions, ImportOptions, Mv3Options, PolOptions, TargetFormat, convert_gltf_to_bytes,
};

use crate::comdef::editor_services::{IImportService, IImportServiceImpl};

use super::project_service::ProjectService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Replace,
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    NotRun = 0,
    Success = 1,
    Partial = 2,
    Failed = 3,
}

struct Converted {
    bytes: Vec<u8>,
    diagnostics: Vec<String>,
}

pub struct ImportService {
    vfs: Rc<MiniFs>,
    catalog: Rc<AssetCatalog>,
    project: ProjectService,

    source_path: RefCell<String>,
    target_format: RefCell<TargetFormat>,
    mode: RefCell<Mode>,

    replace_target_package: RefCell<String>,
    replace_target_internal_path: RefCell<String>,
    replace_template: RefCell<Option<Vec<u8>>>,

    add_target_package: RefCell<String>,
    add_target_directory: RefCell<String>,

    mv3_options: RefCell<Mv3Options>,
    // -1 = unset, 0 = force off, 1 = force on.
    pol_force_use_alpha: RefCell<i32>,
    cvd_legacy_magic: RefCell<bool>,

    save_to_file: RefCell<bool>,
    save_output_path: RefCell<String>,
    add_to_project: RefCell<bool>,

    converted: RefCell<Option<Converted>>,
    status: RefCell<Status>,
    staged_preview_vfs_path: RefCell<String>,

    last_error: RefCell<String>,
    last_string: RefCell<String>,
}

ComObject_ImportService!(super::ImportService);

impl ImportService {
    pub fn create(
        vfs: Rc<MiniFs>,
        catalog: Rc<AssetCatalog>,
        project: ProjectService,
    ) -> ComRc<IImportService> {
        ComRc::from_object(Self {
            vfs,
            catalog,
            project,
            source_path: RefCell::new(String::new()),
            target_format: RefCell::new(TargetFormat::Mv3),
            mode: RefCell::new(Mode::Replace),
            replace_target_package: RefCell::new(String::new()),
            replace_target_internal_path: RefCell::new(String::new()),
            replace_template: RefCell::new(None),
            add_target_package: RefCell::new(String::new()),
            add_target_directory: RefCell::new(String::new()),
            mv3_options: RefCell::new(Mv3Options::default()),
            pol_force_use_alpha: RefCell::new(-1),
            cvd_legacy_magic: RefCell::new(CvdOptions::default().legacy_magic),
            save_to_file: RefCell::new(false),
            save_output_path: RefCell::new(String::new()),
            add_to_project: RefCell::new(false),
            converted: RefCell::new(None),
            status: RefCell::new(Status::NotRun),
            staged_preview_vfs_path: RefCell::new(String::new()),
            last_error: RefCell::new(String::new()),
            last_string: RefCell::new(String::new()),
        })
    }

    fn set_last(&self, s: String) -> &str {
        *self.last_string.borrow_mut() = s;
        // SAFETY: see `ProjectServiceInner::set_last` — single-threaded
        // script/UI path; codegen copies the &str into a CString
        // immediately on return.
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

    /// Invalidates the conversion cache and every downstream
    /// (`status`/`staged_preview_vfs_path`) field. Called by every
    /// setter below.
    fn invalidate_conversion(&self) {
        *self.converted.borrow_mut() = None;
        *self.status.borrow_mut() = Status::NotRun;
        self.staged_preview_vfs_path.borrow_mut().clear();
    }

    fn options(&self) -> ImportOptions {
        ImportOptions {
            mv3: self.mv3_options.borrow().clone(),
            pol: PolOptions {
                force_use_alpha: match *self.pol_force_use_alpha.borrow() {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                },
            },
            cvd: CvdOptions {
                legacy_magic: *self.cvd_legacy_magic.borrow(),
            },
        }
    }

    /// `<source basename>.<target extension>`, or `None` until a source
    /// path is set.
    fn derived_filename(&self) -> Option<String> {
        let source = self.source_path.borrow();
        if source.is_empty() {
            return None;
        }
        let stem = Path::new(&*source)
            .file_stem()?
            .to_string_lossy()
            .to_string();
        Some(format!(
            "{stem}.{}",
            self.target_format.borrow().extension()
        ))
    }

    /// Final Add-mode internal path: `add_target_directory` (trimmed)
    /// joined with `derived_filename()`.
    fn add_internal_path(&self) -> Option<String> {
        let filename = self.derived_filename()?;
        let dir = self.add_target_directory.borrow();
        let dir = dir.trim_matches('/');
        Some(if dir.is_empty() {
            filename
        } else {
            format!("{dir}/{filename}")
        })
    }

    /// Whether `vfs_path` already resolves to something in the base
    /// asset tree (i.e. a real, currently-readable file) — used to
    /// reject an Add-mode target that collides with an existing asset
    /// (which should use Replace instead).
    fn vfs_path_exists(&self, vfs_path: &Path) -> bool {
        self.vfs.open(vfs_path).is_ok()
    }

    fn expected_vfs_path(&self, target_package: &str, internal_path: &str) -> Option<PathBuf> {
        let wanted = target_package.replace('\\', "/");
        self.catalog.mounts().iter().find_map(|mount| {
            let phys = mount
                .physical_relative_path
                .to_string_lossy()
                .replace('\\', "/");
            phys.eq_ignore_ascii_case(&wanted)
                .then(|| mount.vfs_mount_point.join(internal_path))
        })
    }

    fn run_conversion(&self) -> Result<Converted, String> {
        let source_path = self.source_path.borrow().clone();
        if source_path.is_empty() {
            return Err("source_path is not set".to_string());
        }
        let target = *self.target_format.borrow();
        let options = self.options();
        let template = match *self.mode.borrow() {
            Mode::Replace => self.replace_template.borrow().clone(),
            Mode::Add => None,
        };

        convert_gltf_to_bytes(&source_path, target, &options, template.as_deref())
            .map(|(bytes, diagnostics)| Converted {
                bytes,
                diagnostics: diagnostics.messages().map(|m| m.to_string()).collect(),
            })
            .map_err(|e| e.to_string())
    }
}

impl IImportServiceImpl for ImportService {
    fn reset(&self) {
        *self.source_path.borrow_mut() = String::new();
        *self.target_format.borrow_mut() = TargetFormat::Mv3;
        *self.mode.borrow_mut() = Mode::Replace;
        *self.replace_target_package.borrow_mut() = String::new();
        *self.replace_target_internal_path.borrow_mut() = String::new();
        *self.replace_template.borrow_mut() = None;
        *self.add_target_package.borrow_mut() = String::new();
        *self.add_target_directory.borrow_mut() = String::new();
        *self.mv3_options.borrow_mut() = Mv3Options::default();
        *self.pol_force_use_alpha.borrow_mut() = -1;
        *self.cvd_legacy_magic.borrow_mut() = CvdOptions::default().legacy_magic;
        *self.save_to_file.borrow_mut() = false;
        *self.save_output_path.borrow_mut() = String::new();
        *self.add_to_project.borrow_mut() = false;
        self.invalidate_conversion();
        self.last_error.borrow_mut().clear();
    }

    fn set_source_path(&self, path: &str) {
        *self.source_path.borrow_mut() = path.to_string();
        self.invalidate_conversion();
    }

    fn source_path(&self) -> &str {
        let s = self.source_path.borrow().clone();
        self.set_last(s)
    }

    fn set_target_format(&self, format: i32) {
        let format = match format {
            0 => TargetFormat::Mv3,
            1 => TargetFormat::Pol,
            2 => TargetFormat::Cvd,
            _ => return,
        };
        *self.target_format.borrow_mut() = format;
        self.invalidate_conversion();
    }

    fn target_format(&self) -> i32 {
        match *self.target_format.borrow() {
            TargetFormat::Mv3 => 0,
            TargetFormat::Pol => 1,
            TargetFormat::Cvd => 2,
        }
    }

    fn set_mode(&self, mode: i32) {
        let mode = match mode {
            0 => Mode::Replace,
            1 => Mode::Add,
            _ => return,
        };
        *self.mode.borrow_mut() = mode;
        self.invalidate_conversion();
    }

    fn mode(&self) -> i32 {
        match *self.mode.borrow() {
            Mode::Replace => 0,
            Mode::Add => 1,
        }
    }

    fn set_replace_target(&self, vfs_path: &str) -> bool {
        let Some((mount, internal)) = self.catalog.resolve(Path::new(vfs_path)) else {
            self.invalidate_conversion();
            return self.fail(format!("{vfs_path} does not resolve to a mounted package"));
        };
        let template = {
            use std::io::Read;
            let mut file = match self.vfs.open(vfs_path) {
                Ok(f) => f,
                Err(e) => {
                    self.invalidate_conversion();
                    return self.fail(format!("failed to read {vfs_path}: {e}"));
                }
            };
            let mut buf = Vec::new();
            if let Err(e) = file.read_to_end(&mut buf) {
                self.invalidate_conversion();
                return self.fail(format!("failed to read {vfs_path}: {e}"));
            }
            buf
        };

        *self.replace_target_package.borrow_mut() = mount
            .physical_relative_path
            .to_string_lossy()
            .replace('\\', "/");
        *self.replace_target_internal_path.borrow_mut() =
            internal.to_string_lossy().replace('\\', "/");
        *self.replace_template.borrow_mut() = Some(template);
        self.invalidate_conversion();
        self.ok()
    }

    fn replace_target_package(&self) -> &str {
        let s = self.replace_target_package.borrow().clone();
        self.set_last(s)
    }

    fn replace_target_internal_path(&self) -> &str {
        let s = self.replace_target_internal_path.borrow().clone();
        self.set_last(s)
    }

    fn add_target_candidate_count(&self) -> i32 {
        self.catalog
            .mounts()
            .iter()
            .filter(|m| m.package_type == PackageType::Cpk)
            .count() as i32
    }

    fn add_target_candidate_name(&self, idx: i32) -> &str {
        let s = if idx < 0 {
            String::new()
        } else {
            self.catalog
                .mounts()
                .iter()
                .filter(|m| m.package_type == PackageType::Cpk)
                .nth(idx as usize)
                .map(|m| {
                    m.physical_relative_path
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .unwrap_or_default()
        };
        self.set_last(s)
    }

    fn set_add_target_package(&self, target_package: &str) {
        *self.add_target_package.borrow_mut() = target_package.to_string();
        self.invalidate_conversion();
    }

    fn add_target_package(&self) -> &str {
        let s = self.add_target_package.borrow().clone();
        self.set_last(s)
    }

    fn set_add_target_directory(&self, directory: &str) {
        *self.add_target_directory.borrow_mut() = directory.to_string();
        self.invalidate_conversion();
    }

    fn add_target_directory(&self) -> &str {
        let s = self.add_target_directory.borrow().clone();
        self.set_last(s)
    }

    fn set_add_target_directory_from_sibling(&self, vfs_path: &str) -> bool {
        let Some((mount, internal)) = self.catalog.resolve(Path::new(vfs_path)) else {
            return self.fail(format!("{vfs_path} does not resolve to a mounted package"));
        };
        let dir = internal
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        *self.add_target_package.borrow_mut() = mount
            .physical_relative_path
            .to_string_lossy()
            .replace('\\', "/");
        *self.add_target_directory.borrow_mut() = dir;
        self.invalidate_conversion();
        self.ok()
    }

    fn add_target_internal_path(&self) -> &str {
        let s = self.add_internal_path().unwrap_or_default();
        self.set_last(s)
    }

    fn add_target_valid(&self) -> bool {
        if self.add_target_package.borrow().is_empty() {
            return false;
        }
        let Some(internal_path) = self.add_internal_path() else {
            return false;
        };
        if internal_path.is_empty() {
            return false;
        }
        match self.expected_vfs_path(&self.add_target_package.borrow(), &internal_path) {
            Some(vfs_path) => !self.vfs_path_exists(&vfs_path),
            // Target package isn't a live catalog mount (e.g. a new
            // package the project itself would introduce); trackable
            // but not collision-checkable against a live path, so we
            // optimistically allow it.
            None => true,
        }
    }

    fn set_mv3_vertex_scale(&self, scale: f32) {
        self.mv3_options.borrow_mut().vertex_scale = scale;
        self.invalidate_conversion();
    }

    fn set_mv3_ticks_per_second(&self, ticks: f32) {
        self.mv3_options.borrow_mut().ticks_per_second = ticks;
        self.invalidate_conversion();
    }

    fn set_pol_force_use_alpha(&self, mode: i32) {
        *self.pol_force_use_alpha.borrow_mut() = mode;
        self.invalidate_conversion();
    }

    fn set_cvd_legacy_magic(&self, legacy: bool) {
        *self.cvd_legacy_magic.borrow_mut() = legacy;
        self.invalidate_conversion();
    }

    fn set_save_to_file(&self, enabled: bool) {
        *self.save_to_file.borrow_mut() = enabled;
    }

    fn save_to_file_enabled(&self) -> bool {
        *self.save_to_file.borrow()
    }

    fn set_save_output_path(&self, path: &str) {
        *self.save_output_path.borrow_mut() = path.to_string();
    }

    fn save_output_path(&self) -> &str {
        let s = self.save_output_path.borrow().clone();
        self.set_last(s)
    }

    fn set_add_to_project(&self, enabled: bool) {
        *self.add_to_project.borrow_mut() = enabled;
    }

    fn add_to_project_enabled(&self) -> bool {
        *self.add_to_project.borrow()
    }

    fn convert(&self) -> bool {
        if self.converted.borrow().is_some() {
            return self.ok();
        }
        match self.run_conversion() {
            Ok(converted) => {
                *self.converted.borrow_mut() = Some(converted);
                self.ok()
            }
            Err(e) => {
                *self.status.borrow_mut() = Status::Failed;
                self.fail(e)
            }
        }
    }

    fn has_converted(&self) -> bool {
        self.converted.borrow().is_some()
    }

    fn diagnostic_count(&self) -> i32 {
        self.converted
            .borrow()
            .as_ref()
            .map(|c| c.diagnostics.len() as i32)
            .unwrap_or(0)
    }

    fn diagnostic(&self, index: i32) -> &str {
        let s = if index < 0 {
            String::new()
        } else {
            self.converted
                .borrow()
                .as_ref()
                .and_then(|c| c.diagnostics.get(index as usize).cloned())
                .unwrap_or_default()
        };
        self.set_last(s)
    }

    fn converted_size(&self) -> i32 {
        self.converted
            .borrow()
            .as_ref()
            .map(|c| c.bytes.len().min(i32::MAX as usize) as i32)
            .unwrap_or(0)
    }

    fn run(&self) -> bool {
        let save = *self.save_to_file.borrow();
        let stage = *self.add_to_project.borrow();
        if !save && !stage {
            *self.status.borrow_mut() = Status::Failed;
            return self.fail("no output selected: enable save_to_file and/or add_to_project");
        }

        if self.converted.borrow().is_none() {
            match self.run_conversion() {
                Ok(converted) => *self.converted.borrow_mut() = Some(converted),
                Err(e) => {
                    *self.status.borrow_mut() = Status::Failed;
                    return self.fail(e);
                }
            }
        }

        let bytes = self
            .converted
            .borrow()
            .as_ref()
            .map(|c| c.bytes.clone())
            .expect("converted bytes populated above");

        let mut errors: Vec<String> = Vec::new();
        let mut successes = 0usize;
        let mut enabled = 0usize;

        if save {
            enabled += 1;
            let out_path = self.save_output_path.borrow().clone();
            if out_path.is_empty() {
                errors.push("save_output_path is not set".to_string());
            } else {
                match std::fs::write(&out_path, &bytes) {
                    Ok(()) => successes += 1,
                    Err(e) => errors.push(format!("failed to write {out_path}: {e}")),
                }
            }
        }

        if stage {
            enabled += 1;
            match self.stage_to_project(&bytes) {
                Ok(preview_path) => {
                    successes += 1;
                    *self.staged_preview_vfs_path.borrow_mut() = preview_path.unwrap_or_default();
                }
                Err(e) => errors.push(e),
            }
        }

        *self.status.borrow_mut() = if successes == enabled {
            Status::Success
        } else if successes > 0 {
            Status::Partial
        } else {
            Status::Failed
        };

        if errors.is_empty() {
            self.ok()
        } else {
            self.fail(errors.join("; "))
        }
    }

    fn status(&self) -> i32 {
        *self.status.borrow() as i32
    }

    fn staged_preview_vfs_path(&self) -> &str {
        let s = self.staged_preview_vfs_path.borrow().clone();
        self.set_last(s)
    }

    fn last_error(&self) -> &str {
        let s = self.last_error.borrow().clone();
        self.set_last(s)
    }
}

impl ImportService {
    /// Stages `bytes` into the active project, targeting either the
    /// resolved Replace target or the derived Add target (whichever
    /// `mode()` currently is), returning the vfs path a preview could
    /// open (`Some`) if the target package is a live catalog mount, or
    /// `Ok(None)` if it isn't (the change is still staged/publishable —
    /// see `ProjectServiceInner::resolve_vfs_path`'s doc comment).
    fn stage_to_project(&self, bytes: &[u8]) -> Result<Option<String>, String> {
        let (target_package, internal_path) = match *self.mode.borrow() {
            Mode::Replace => {
                let package = self.replace_target_package.borrow().clone();
                let internal = self.replace_target_internal_path.borrow().clone();
                if package.is_empty() || internal.is_empty() {
                    return Err("no replace target set".to_string());
                }
                (package, internal)
            }
            Mode::Add => {
                if !self.add_target_valid() {
                    return Err(
                        "add target is invalid: choose a target package/directory that doesn't \
                         already exist (or use Replace mode)"
                            .to_string(),
                    );
                }
                let package = self.add_target_package.borrow().clone();
                let internal = self
                    .add_internal_path()
                    .ok_or_else(|| "add target internal path could not be derived".to_string())?;
                (package, internal)
            }
        };

        let source_path = self.source_path.borrow().clone();
        self.project.stage_payload_bytes(
            &target_package,
            &internal_path,
            bytes,
            &source_path,
            "yaobow_editor.import_wizard",
            env!("CARGO_PKG_VERSION"),
        )?;

        Ok(self
            .project
            .vfs_path_for_parts(
                &asset_project::TargetPackage::new(&target_package)
                    .map_err(|e| format!("invalid target_package: {e}"))?,
                &asset_project::PackagePath::new(&internal_path)
                    .map_err(|e| format!("invalid internal_path: {e}"))?,
            )
            .map(|p| p.to_string_lossy().replace('\\', "/")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comdef::editor_services::{
        IAudioHandle, IImageHandle, IModelHandle, IPreviewerHub, IPreviewerHubImpl,
        IProjectService, ISceneHandle,
    };
    use crate::comdef::services::IUiLayoutHandle;
    use crate::services::project_overlay;
    use crate::services::resource_manager::ResourceManager;
    use shared::GameType;
    use std::io::Write as _;

    struct StubPreviewerHub;
    ComObject_PreviewerHub!(crate::services::import_service::tests::StubPreviewerHub);
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

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!(
                "import-{name}-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Builds a minimal, spec-valid single-triangle `.glb` (POSITION +
    /// TEXCOORD_0 + indices, no material/normals — the smallest input
    /// `shared::importers::pol::convert_with_template` accepts) purely
    /// via the `gltf` crate's own `Glb` writer, mirroring
    /// `shared::importers::test_support::SceneBuilder` (which is
    /// private to `shared` and not reusable from here) but hand-rolled
    /// locally so this test module has no dependency on it.
    fn triangle_glb_bytes() -> Vec<u8> {
        use gltf::binary::{Glb, Header};
        use serde_json::json;
        use std::borrow::Cow;

        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uvs: [[f32; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let indices: [u16; 3] = [0, 1, 2];

        let mut bin = Vec::new();
        let pos_offset = bin.len();
        for p in &positions {
            for f in p {
                bin.extend_from_slice(&f.to_le_bytes());
            }
        }
        let pos_len = bin.len() - pos_offset;
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let uv_offset = bin.len();
        for uv in &uvs {
            for f in uv {
                bin.extend_from_slice(&f.to_le_bytes());
            }
        }
        let uv_len = bin.len() - uv_offset;
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let idx_offset = bin.len();
        for i in &indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let idx_len = bin.len() - idx_offset;
        while bin.len() % 4 != 0 {
            bin.push(0);
        }

        let root = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": bin.len() }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 },
                { "buffer": 0, "byteOffset": uv_offset, "byteLength": uv_len, "target": 34962 },
                { "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 },
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
                { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2" },
                { "bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR" },
            ],
            "meshes": [{
                "primitives": [{
                    "attributes": { "POSITION": 0, "TEXCOORD_0": 1 },
                    "indices": 2,
                    "mode": 4,
                }]
            }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0,
        });

        let json_bytes = serde_json::to_vec(&root).expect("serializing synthetic glTF JSON");
        let glb = Glb {
            header: Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: Cow::Owned(json_bytes),
            bin: Some(Cow::Owned(bin)),
        };
        glb.to_vec().expect("assembling synthetic GLB")
    }

    fn write_glb_fixture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, triangle_glb_bytes()).unwrap();
        path
    }

    fn make_project(base_asset_root: PathBuf) -> (ProjectService, ComRc<IProjectService>) {
        let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub);
        ProjectService::create(
            GameType::PAL3,
            base_asset_root,
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_overlay::new_shared_overlay_index(),
            previewers,
        )
    }

    fn make_import_service_with_project(
        vfs: Rc<MiniFs>,
        catalog: Rc<AssetCatalog>,
        project: ProjectService,
    ) -> ComRc<IImportService> {
        ImportService::create(vfs, catalog, project)
    }

    fn make_import_service() -> ComRc<IImportService> {
        make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            make_project(PathBuf::from("/base")).0,
        )
    }

    #[test]
    fn reset_clears_every_field() {
        let root = scratch_dir("reset");
        let glb = write_glb_fixture(&root, "model.glb");
        let svc = make_import_service();

        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");
        svc.set_add_target_directory("q01");
        svc.set_save_to_file(true);
        svc.set_save_output_path("/tmp/whatever.pol");
        svc.set_add_to_project(true);
        assert!(svc.convert());
        assert!(svc.has_converted());

        svc.reset();

        assert_eq!(svc.source_path(), "");
        assert_eq!(svc.target_format(), 0);
        assert_eq!(svc.mode(), 0);
        assert_eq!(svc.add_target_package(), "");
        assert_eq!(svc.add_target_directory(), "");
        assert!(!svc.save_to_file_enabled());
        assert_eq!(svc.save_output_path(), "");
        assert!(!svc.add_to_project_enabled());
        assert!(!svc.has_converted());
        assert_eq!(svc.status(), 0);
        assert_eq!(svc.staged_preview_vfs_path(), "");
    }

    #[test]
    fn set_replace_target_fails_for_unresolved_vfs_path() {
        let svc = make_import_service();
        assert!(!svc.set_replace_target("/scene/does_not_exist/q01.pol"));
        assert!(!svc.last_error().is_empty());
        assert_eq!(svc.replace_target_package(), "");
    }

    #[test]
    fn add_target_invalid_when_no_package_selected() {
        let root = scratch_dir("add-invalid-no-pkg");
        let glb = write_glb_fixture(&root, "model.glb");
        let svc = make_import_service();
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        // No `set_add_target_package` call: must be invalid.
        assert!(!svc.add_target_valid());
    }

    #[test]
    fn run_fails_when_no_output_selected() {
        let root = scratch_dir("no-output");
        let glb = write_glb_fixture(&root, "model.glb");
        let svc = make_import_service();
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");

        assert!(!svc.run());
        assert_eq!(svc.status(), 3); // Failed
        assert!(!svc.last_error().is_empty());
    }

    #[test]
    fn run_save_only_writes_file_and_does_not_touch_project() {
        let root = scratch_dir("save-only");
        let glb = write_glb_fixture(&root, "model.glb");
        let out_path = root.join("out.pol");
        let (project_handle, project) = make_project(root.join("base"));
        assert!(!project.has_active_project());

        let svc = make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_handle.clone(),
        );
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_save_to_file(true);
        svc.set_save_output_path(out_path.to_str().unwrap());

        assert!(svc.run());
        assert_eq!(svc.status(), 1); // Success
        assert!(out_path.exists());
        assert!(std::fs::metadata(&out_path).unwrap().len() > 0);
        // Save-only: no project involvement, none expected/possible.
        assert!(!project.has_active_project());
        assert_eq!(svc.staged_preview_vfs_path(), "");
    }

    #[test]
    fn run_project_only_stages_change_without_writing_file() {
        let root = scratch_dir("project-only");
        let glb = write_glb_fixture(&root, "model.glb");
        let (project_handle, project) = make_project(root.join("base"));
        assert!(project.create_project(root.join("proj").to_str().unwrap()));

        let svc = make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_handle.clone(),
        );
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");
        svc.set_add_target_directory("q01");
        svc.set_add_to_project(true);

        assert!(svc.run());
        assert_eq!(svc.status(), 1);
        assert_eq!(project.change_count(), 1);
        assert_eq!(project.change_target_package(0), "scene/q01.cpk");
        assert_eq!(project.change_internal_path(0), "q01/model.pol");
        // No live catalog mount for "scene/q01.cpk" in this test, so the
        // change is trackable/publishable but not live-previewable.
        assert_eq!(svc.staged_preview_vfs_path(), "");
    }

    #[test]
    fn run_dual_output_converts_once_and_routes_same_bytes_to_both() {
        let root = scratch_dir("dual-output");
        let glb = write_glb_fixture(&root, "model.glb");
        let out_path = root.join("out.pol");
        let (project_handle, project) = make_project(root.join("base"));
        assert!(project.create_project(root.join("proj").to_str().unwrap()));

        let svc = make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_handle.clone(),
        );
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");
        svc.set_save_to_file(true);
        svc.set_save_output_path(out_path.to_str().unwrap());
        svc.set_add_to_project(true);

        assert!(svc.run());
        assert_eq!(svc.status(), 1);
        assert_eq!(project.change_count(), 1);

        let written = std::fs::read(&out_path).unwrap();
        let staged_size = project.change_payload_size(0) as usize;
        assert_eq!(written.len(), staged_size);
        assert_eq!(written.len(), svc.converted_size() as usize);
    }

    #[test]
    fn run_fails_gracefully_when_staging_without_active_project() {
        let root = scratch_dir("staging-failure");
        let glb = write_glb_fixture(&root, "model.glb");
        let (project_handle, project) = make_project(root.join("base"));
        assert!(!project.has_active_project());

        let svc = make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_handle,
        );
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");

        svc.set_add_to_project(true);
        assert!(!svc.run());
        assert_eq!(svc.status(), 3); // Failed: conversion succeeded but the
        // only enabled output (staging) failed outright.
        assert!(svc.last_error().contains("no active project"));
    }

    #[test]
    fn run_reports_partial_status_when_one_of_two_outputs_fails() {
        let root = scratch_dir("partial");
        let glb = write_glb_fixture(&root, "model.glb");
        let (project_handle, project) = make_project(root.join("base"));
        assert!(project.create_project(root.join("proj").to_str().unwrap()));

        let svc = make_import_service_with_project(
            Rc::new(MiniFs::new(false)),
            Rc::new(AssetCatalog::new(PathBuf::new())),
            project_handle.clone(),
        );
        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(1);
        svc.set_add_target_package("scene/q01.cpk");
        svc.set_add_to_project(true);
        svc.set_save_to_file(true);
        // A directory (not a file) as the save path always fails to
        // open for writing, letting the "add to project" output
        // succeed independently.
        svc.set_save_output_path(root.to_str().unwrap());

        assert!(!svc.run());
        assert_eq!(svc.status(), 2); // Partial
        assert_eq!(project.change_count(), 1);
    }

    /// Builds a minimal on-disk `.zip` package (readable by the
    /// vendored `mini_fs::ZipFs` `packfs` mounts `.zip` files with) at
    /// `<dir>/<name>.zip`, containing a single entry `entry_name` with
    /// `data`, then mounts `dir` via
    /// `packfs::init_virtual_fs_with_catalog` so tests get a *real*,
    /// catalog-resolvable vfs path without any packfs changes.
    fn write_zip_package(dir: &Path, name: &str, entry_name: &str, data: &[u8]) {
        let path = dir.join(format!("{name}.zip"));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file(entry_name, options).unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn set_replace_target_resolves_via_catalog_and_uses_original_as_template() {
        let root = scratch_dir("template-replace");
        let glb = write_glb_fixture(&root, "model.glb");

        // First conversion (no template) produces the "existing" bytes
        // this test replaces.
        let base_bytes = shared::importers::convert_gltf_to_bytes(
            &glb,
            TargetFormat::Pol,
            &ImportOptions::default(),
            None,
        )
        .expect("initial conversion should succeed")
        .0;

        let pkg_dir = root.join("pkg");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        write_zip_package(&pkg_dir, "data", "model.pol", &base_bytes);

        let (vfs, catalog) = packfs::init_virtual_fs_with_catalog(&pkg_dir, None);
        let vfs = Rc::new(vfs);
        let catalog = Rc::new(catalog);

        let (project_handle, _project) = make_project(root.join("base"));
        let svc = make_import_service_with_project(vfs, catalog, project_handle);

        svc.set_source_path(glb.to_str().unwrap());
        svc.set_target_format(1);
        svc.set_mode(0);

        assert!(svc.set_replace_target("/data/model.pol"));
        assert_eq!(svc.replace_target_package(), "data.zip");
        assert_eq!(svc.replace_target_internal_path(), "model.pol");

        assert!(svc.convert());
        // The synthetic source glTF has no `asset.extras.yaobow`
        // round-trip payload, so `pol::convert_with_template` must have
        // fallen back to the replacement template's opaque metadata —
        // only possible if `set_replace_target` actually read
        // `base_bytes` and threaded them through as the template.
        let mut found_template_diagnostic = false;
        for i in 0..svc.diagnostic_count() {
            if svc.diagnostic(i).contains("template") {
                found_template_diagnostic = true;
            }
        }
        assert!(
            found_template_diagnostic,
            "expected a template-fallback diagnostic"
        );
    }
}
