//! Compile + smoke tests for `scripts/import_wizard.p7` (the unified
//! glTF import wizard UI, backed by the host-side `IImportService`).
//!
//! Mirrors `project_panel_script_compile.rs`'s structure and the same
//! `RecordingUiHost` limitation it documents: `checkbox`/`slider_int`
//! echo back the value they're passed (no click simulation), and
//! `list_clipped` invokes its body once with `list_clipped_index() ==
//! -1`. So these tests drive wizard *state* transitions directly
//! through the real `IImportService`/`IProjectService` Rust surface
//! (exactly as a p7 setter call would) and then assert the wizard
//! renders the expected labels/text for that state.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance::comdef::ISceneManager;
use radiance::comdef::IUiHost;
use radiance_scripting::comdef::services::{
    IAppService, IAudioService, IConfigService, IGameRegistry, IHostContextImpl, IInputService,
    IRandomService, ITextureService, IVfsService,
};
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::services::{GameRegistry, RandomService};
use yaobow_editor::comdef::editor_services::{
    IEditorHostContext, IEditorHostContextImpl, IImportService, IPreviewerHub, IPreviewerHubImpl,
    IProjectService, IProjectServiceImpl,
};

mod comdef {
    pub use yaobow_editor::comdef::*;
}

/// Minimal probe script exercising `import_wizard.p7` without pulling
/// in the rest of `main_editor.p7`. Exposes the wizard's entry points
/// as top-level `init_add`/`init_replace` constructors (rather
/// than instance methods — see `Harness.render`'s doc comment) so the
/// Rust test harness can drive them the same way `main_editor.p7`
/// does, and repurposes `render`'s `int` return to report the current
/// `picks.len()` (the "preview staged asset" routing target) without
/// needing a full `MainEditorDirector`.
const PROBE_SRC: &str = r#"
import radiance;
import yaobow_editor.yaobow_editor_services;
import yaobow_editor.import_wizard;
import yaobow_editor.main_editor;
import yaobow_editor.content_tabs;

pub struct[radiance.IUiLayer, radiance.IDirector] Harness(
    pub host: box<yaobow_editor_services.IEditorHostContext>,
    pub state: box<import_wizard.ImportWizardState>,
    pub picks: box<array<string>>,
) {
    pub fn activate(self: refmut<Self>) -> int { 0 }
    pub fn deactivate(self: refmut<Self>) -> int { 0 }

    // Returns the current `picks` length so Rust-side tests can
    // observe the "Preview staged asset" button's routing without
    // needing an interface-vtable method beyond the declared
    // `IUiLayer`/`IDirector` surface (custom instance methods aren't
    // reachable via `ScriptHost::call_method_*` — only methods on a
    // proto box's declared interfaces are).
    pub fn render(self: refmut<Self>, ui: box<radiance.IUiHost>, dt: float) -> int {
        import_wizard.render_import_wizard(ui, self.host, self.state, self.picks);
        return self.picks.len();
    }

    pub fn update(self: refmut<Self>, dt: float) -> ?box<radiance.IDirector> {
        return null;
    }
}

// Four top-level constructors (rather than instance methods — see
// `render`'s doc comment above) covering the wizard's entry points:
// closed by default, "already open" against whatever the service's
// current state already is (for tests that pre-configure the service
// directly, mirroring mid-flow usage rather than a fresh open), the
// fixed Add entry seeded from a current-preview context path, and the
// model/resource-context
// "Import glTF (Replace)..." entry.
pub fn init(host: box<yaobow_editor_services.IEditorHostContext>) -> box<radiance.IDirector> {
    let picks: array<string> = [];
    return box(Harness(host, box(import_wizard.make_import_wizard_state()), box(picks)));
}

pub fn init_shown(host: box<yaobow_editor_services.IEditorHostContext>) -> box<radiance.IDirector> {
    let picks: array<string> = [];
    let state = box(import_wizard.make_import_wizard_state());
    state.show = true;
    return box(Harness(host, state, box(picks)));
}

pub fn init_add(
    host: box<yaobow_editor_services.IEditorHostContext>,
    context_vfs_path: string,
) -> box<radiance.IDirector> {
    let picks: array<string> = [];
    let state = box(import_wizard.make_import_wizard_state());
    import_wizard.open_wizard_for_add(state, host.imports(), context_vfs_path);
    return box(Harness(host, state, box(picks)));
}

pub fn init_replace(
    host: box<yaobow_editor_services.IEditorHostContext>,
    vfs_path: string,
) -> box<radiance.IDirector> {
    let picks: array<string> = [];
    let state = box(import_wizard.make_import_wizard_state());
    import_wizard.open_wizard_for_replace(state, host.imports(), vfs_path);
    return box(Harness(host, state, box(picks)));
}

pub fn render_menu_import_entry(
    ui: box<radiance.IUiHost>,
    host: box<yaobow_editor_services.IEditorHostContext>,
    context_vfs_path: string,
) -> int {
    let state = box(import_wizard.make_import_wizard_state());
    main_editor.render_import_menu_item(
        ui,
        host.project(),
        host.imports(),
        state,
        context_vfs_path,
    );
    if state.show {
        return 1;
    }
    return 0;
}

pub fn render_preview_import_entry(
    ui: box<radiance.IUiHost>,
    host: box<yaobow_editor_services.IEditorHostContext>,
    vfs_path: string,
) -> int {
    let intents: array<string> = [];
    let boxed_intents = box(intents);
    content_tabs.render_import_replace_button(ui, host.project(), vfs_path, boxed_intents);
    return boxed_intents.len();
}
"#;

fn build_test_assets() -> Rc<radiance::asset::AssetManager> {
    let assets = radiance::asset::AssetManager::new();
    radiance_scripting::mount_engine_bindings(&assets);
    radiance_scripting::mount_scripts(&assets);
    shared::mount_scripts(&assets);
    yaobow_editor::script_source::mount_scripts(&assets);
    assets
}

/// `import_wizard.p7` in isolation: compiles and resolves its imports
/// against the same module provider used in production.
#[test]
fn import_wizard_module_compiles_standalone() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("import_wizard.p7 should compile and resolve");
}

struct StubPreviewerHub;
yaobow_editor::ComObject_PreviewerHub!(crate::StubPreviewerHub);
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
    fn open_image(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::editor_services::IImageHandle>> {
        None
    }
    fn open_audio(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::editor_services::IAudioHandle>> {
        None
    }
    fn open_video(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::services::IVideoHandle>> {
        None
    }
    fn open_model(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::editor_services::IModelHandle>> {
        None
    }
    fn open_scene(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::editor_services::ISceneHandle>> {
        None
    }
    fn open_ui_layout(
        &self,
        _vfs_path: &str,
    ) -> Option<ComRc<yaobow_editor::comdef::services::IUiLayoutHandle>> {
        None
    }
    fn resources(&self) -> ComRc<yaobow_editor::comdef::editor_services::IResourceManager> {
        yaobow_editor::services::ResourceManager::create(Rc::new(mini_fs::MiniFs::new(false)))
    }
}

struct TestHostContext {
    config: ComRc<IConfigService>,
    project: ComRc<IProjectService>,
    imports: ComRc<IImportService>,
}

yaobow_editor::ComObject_EditorHostContext!(crate::TestHostContext);

impl IHostContextImpl for TestHostContext {
    fn scene_manager(&self) -> ComRc<ISceneManager> {
        panic!("scene_manager should not be called by the import wizard smoke test")
    }
    fn audio(&self) -> ComRc<IAudioService> {
        panic!("audio should not be called by the import wizard smoke test")
    }
    fn textures(&self) -> ComRc<ITextureService> {
        panic!("textures should not be called by the import wizard smoke test")
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        panic!("vfs should not be called by the import wizard smoke test")
    }
    fn input(&self) -> ComRc<IInputService> {
        panic!("input should not be called by the import wizard smoke test")
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        GameRegistry::create()
    }
    fn app(&self) -> ComRc<IAppService> {
        panic!("app should not be called by the import wizard smoke test")
    }
    fn random(&self) -> ComRc<IRandomService> {
        RandomService::create()
    }
    fn config(&self) -> ComRc<IConfigService> {
        self.config.clone()
    }
}

impl IEditorHostContextImpl for TestHostContext {
    fn previewers(&self) -> ComRc<IPreviewerHub> {
        panic!("previewers should not be called by the import wizard smoke test")
    }
    fn project(&self) -> ComRc<IProjectService> {
        self.project.clone()
    }
    fn imports(&self) -> ComRc<IImportService> {
        self.imports.clone()
    }
    fn new_render_target(
        &self,
        _w: i32,
        _h: i32,
    ) -> ComRc<radiance_scripting::comdef::services::IRenderTarget> {
        panic!("not used by import_wizard_script_compile")
    }
    fn render_pending_previews(&self) {}
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
            "wizard-{name}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Minimal single-triangle `.glb` (POSITION + TEXCOORD_0 + indices),
/// built directly via the `gltf` crate's `Glb` writer — the same
/// hand-rolled approach used by `src/services/import_service.rs`'s own
/// test module (duplicated here since that module is private and this
/// is a separate integration-test crate).
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

fn make_project_pair(
    base_asset_root: PathBuf,
) -> (
    yaobow_editor::services::ProjectService,
    ComRc<IProjectService>,
) {
    let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub);
    yaobow_editor::services::ProjectService::create(
        shared::GameType::PAL3,
        base_asset_root,
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(PathBuf::new())),
        yaobow_editor::services::project_overlay::new_shared_overlay_index(),
        previewers,
    )
}

fn make_project_service(base_asset_root: PathBuf) -> yaobow_editor::services::ProjectService {
    make_project_pair(base_asset_root).0
}

fn make_import_service(project: yaobow_editor::services::ProjectService) -> ComRc<IImportService> {
    yaobow_editor::services::ImportService::create(
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(PathBuf::new())),
        project,
    )
}

fn make_config_service() -> ComRc<IConfigService> {
    shared::config_service::ConfigService::create(Rc::new(RefCell::new(
        shared::config::YaobowConfig::default(),
    )))
}

struct Env {
    runtime: Rc<radiance_scripting::ScriptHost>,
    handle: radiance_scripting::ScriptDirectorHandle,
}

fn host_foreign_box(
    runtime: &radiance_scripting::ScriptHost,
    imports: ComRc<IImportService>,
) -> Data {
    let (_, project_com) = yaobow_editor::services::ProjectService::create(
        shared::GameType::PAL3,
        PathBuf::new(),
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(PathBuf::new())),
        yaobow_editor::services::project_overlay::new_shared_overlay_index(),
        ComRc::<IPreviewerHub>::from_object(StubPreviewerHub),
    );
    host_foreign_box_with_project(runtime, project_com, imports)
}

fn host_foreign_box_with_project(
    runtime: &radiance_scripting::ScriptHost,
    project: ComRc<IProjectService>,
    imports: ComRc<IImportService>,
) -> Data {
    let host_ctx = ComRc::<IEditorHostContext>::from_object(TestHostContext {
        config: make_config_service(),
        project,
        imports,
    });
    let host_id = runtime.intern(host_ctx);
    runtime
        .foreign_box(
            "yaobow_editor.comdef.editor_services.IEditorHostContext",
            host_id,
        )
        .expect("host foreign box")
}

fn call_entry(
    runtime: &radiance_scripting::ScriptHost,
    name: &str,
    ui: ComRc<IUiHost>,
    host: Data,
    path: &str,
) -> i64 {
    let ui_id = runtime.intern(ui);
    let ui = runtime
        .foreign_box("radiance.comdef.IUiHost", ui_id)
        .expect("ui foreign box");
    match runtime
        .call_returning_data(name, vec![ui, host, Data::String(Rc::from(path))])
        .expect("entry runs")
    {
        Data::Int(value) => value,
        other => panic!("expected int result, got {other:?}"),
    }
}

/// Wizard closed by default (no context, no entry point taken).
fn init_env(imports: ComRc<IImportService>) -> Env {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("import_wizard.p7 probe script should compile");

    let host = host_foreign_box(&runtime, imports);
    let state = runtime
        .call_returning_data("init", vec![host])
        .expect("probe init should run");
    let handle = runtime.root(state);
    Env { runtime, handle }
}

/// Wizard already shown against whatever state the caller has already
/// configured directly on `imports` — unlike `init_env_generic`, this
/// does *not* call `open_wizard` (which resets the service state), so
/// it's used by tests that pre-configure and run a conversion before
/// inspecting the rendered wizard.
fn init_env_shown(imports: ComRc<IImportService>) -> Env {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("import_wizard.p7 probe script should compile");

    let host = host_foreign_box(&runtime, imports);
    let state = runtime
        .call_returning_data("init_shown", vec![host])
        .expect("probe init_shown should run");
    let handle = runtime.root(state);
    Env { runtime, handle }
}

/// Fixed Add entry point seeded from the current preview path.
fn init_env_add(imports: ComRc<IImportService>, context_vfs_path: &str) -> Env {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("import_wizard.p7 probe script should compile");

    let host = host_foreign_box(&runtime, imports);
    let state = runtime
        .call_returning_data(
            "init_add",
            vec![host, Data::String(Rc::from(context_vfs_path))],
        )
        .expect("probe init_add should run");
    let handle = runtime.root(state);
    Env { runtime, handle }
}

/// Model/resource-context "Import glTF (Replace)..." entry point.
fn init_env_replace(imports: ComRc<IImportService>, vfs_path: &str) -> Env {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("import_wizard.p7 probe script should compile");

    let host = host_foreign_box(&runtime, imports);
    let state = runtime
        .call_returning_data("init_replace", vec![host, Data::String(Rc::from(vfs_path))])
        .expect("probe init_replace should run");
    let handle = runtime.root(state);
    Env { runtime, handle }
}

fn director(env: &Env) -> Data {
    env.runtime
        .deref_handle(env.handle)
        .expect("harness director should be rooted")
}

/// Renders one frame and returns the harness's reported `picks.len()`
/// (`render`'s return value — see the probe script's doc comment).
fn render(env: &Env, ui_com: ComRc<IUiHost>) -> i64 {
    let director = director(env);
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box("radiance.comdef.IUiHost", ui_com_id)
        .expect("ui_host foreign box");
    match env
        .runtime
        .call_method_returning_data(director, "render", vec![ui_box, Data::Float(0.0)])
        .expect("harness render should run")
    {
        Data::Int(n) => n,
        other => panic!("expected Data::Int from render, got {other:?}"),
    }
}

#[test]
fn import_wizard_hidden_by_default_renders_nothing() {
    let project = make_project_service(PathBuf::from("/base"));
    let imports = make_import_service(project);
    let env = init_env(imports);
    let (recorder, ui_com) = RecordingUiHost::create();

    render(&env, ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, UiCall::WindowCentered { .. })),
        "expected no window when the wizard is closed, got {calls:?}"
    );
}

#[test]
fn import_wizard_add_entry_uses_preview_parent_and_hides_mode_and_outputs() {
    let root = scratch_dir("add-entry");
    let pkg_dir = root.join("pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    {
        use std::io::Write;
        let file = std::fs::File::create(pkg_dir.join("data.zip")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("models/current.pol", options).unwrap();
        zip.write_all(b"current model").unwrap();
        zip.finish().unwrap();
    }

    let (vfs, catalog) = packfs::init_virtual_fs_with_catalog(&pkg_dir, None);
    let project = make_project_service(root.join("base"));
    let imports =
        yaobow_editor::services::ImportService::create(Rc::new(vfs), Rc::new(catalog), project);
    let env = init_env_add(imports.clone(), "/data/models/current.pol");

    assert_eq!(imports.mode(), 1);
    assert_eq!(imports.add_target_package(), "data.zip");
    assert_eq!(imports.add_target_directory(), "models");
    assert!(imports.add_to_project_enabled());
    assert!(!imports.save_to_file_enabled());

    let (recorder, ui_com) = RecordingUiHost::create();
    render(&env, ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        calls.iter().any(
            |c| matches!(c, UiCall::WindowCentered { title, .. } if title.starts_with("Import glTF"))
        ),
        "expected the wizard window once opened, got {calls:?}"
    );

    let expected_tree_leaves = [
        "MV3 (skeletal / character model)",
        "POL (static model)",
        "CVD (prop model)",
    ];
    for label in expected_tree_leaves {
        assert!(
            calls
                .iter()
                .any(|c| matches!(c, UiCall::TreeLeaf { label: l, .. } if l == label)),
            "expected a \"{label}\" TreeLeaf call, got {calls:?}"
        );
    }

    let expected_buttons = [
        "Choose .glb file...",
        "Choose .gltf file...",
        "Convert (preview)",
        "Run",
        "Cancel",
        "Close",
    ];
    for label in expected_buttons {
        assert!(
            calls
                .iter()
                .any(|c| matches!(c, UiCall::Button { label: l, .. } if l == label)),
            "expected a \"{label}\" Button call, got {calls:?}"
        );
    }
    for label in ["Run", "Cancel", "Close"] {
        assert!(
            calls.iter().any(
                |c| matches!(c, UiCall::Button { label: l, w, h } if l == label && *w == 100.0 && *h == 28.0)
            ),
            "expected a consistently sized \"{label}\" button, got {calls:?}"
        );
    }
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "Target package: data.zip")),
        "expected the derived package, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "Target directory: models")),
        "expected the derived directory, got {calls:?}"
    );
    assert!(
        !calls.iter().any(
            |c| matches!(c, UiCall::TreeLeaf { label, .. } if label == "Replace an existing asset" || label == "Add a new asset")
        ),
        "fixed Add entry must omit mode choices, got {calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, UiCall::Checkbox { label, .. }
            if label.contains("active project") || label.contains("disk"))),
        "editor import must omit output choices, got {calls:?}"
    );
}

#[test]
fn import_entry_points_are_disabled_without_an_active_project_or_preview() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime.load_source(PROBE_SRC).expect("probe compiles");

    let (project, project_com) = make_project_pair(PathBuf::from("/base"));
    let imports = make_import_service(project);
    let host = host_foreign_box_with_project(&runtime, project_com, imports);
    let (recorder, ui) = RecordingUiHost::create();
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("Import glTF to current folder".to_string(), true);
    assert_eq!(
        call_entry(
            &runtime,
            "render_menu_import_entry",
            ui,
            host,
            "/data/model.pol",
        ),
        0
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(
        call,
        UiCall::MenuItemEnabled { label, enabled: false, .. }
            if label == "Import glTF to current folder"
    )));

    let root = scratch_dir("no-preview-entry");
    let (project, project_com) = make_project_pair(root.join("base"));
    assert!(project.create_project(root.join("proj").to_str().unwrap()));
    let imports = make_import_service(project);
    let host = host_foreign_box_with_project(&runtime, project_com, imports);
    let (recorder, ui) = RecordingUiHost::create();
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("Import glTF to current folder".to_string(), true);
    assert_eq!(
        call_entry(&runtime, "render_menu_import_entry", ui, host, ""),
        0
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(
        call,
        UiCall::MenuItemEnabled { label, enabled: false, .. }
            if label == "Import glTF to current folder"
    )));

    let (project, project_com) = make_project_pair(PathBuf::from("/base"));
    let imports = make_import_service(project);
    let host = host_foreign_box_with_project(&runtime, project_com, imports);
    let (recorder, ui) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("Import glTF (Replace)...".to_string(), true);
    assert_eq!(
        call_entry(
            &runtime,
            "render_preview_import_entry",
            ui,
            host,
            "/data/model.pol",
        ),
        0
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(
        call,
        UiCall::ButtonEnabled { label, enabled: false, .. }
            if label == "Import glTF (Replace)..."
    )));
}

#[test]
fn import_entry_points_enable_with_an_active_project_and_preview() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime.load_source(PROBE_SRC).expect("probe compiles");

    let root = scratch_dir("enabled-entries");
    let (project, project_com) = make_project_pair(root.join("base"));
    assert!(project.create_project(root.join("proj").to_str().unwrap()));
    let imports = make_import_service(project.clone());
    let host = host_foreign_box_with_project(&runtime, project_com.clone(), imports);
    let (recorder, ui) = RecordingUiHost::create();
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("Import glTF to current folder".to_string(), true);
    assert_eq!(
        call_entry(
            &runtime,
            "render_menu_import_entry",
            ui,
            host,
            "/data/model.pol",
        ),
        1
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(
        call,
        UiCall::MenuItemEnabled { label, enabled: true, .. }
            if label == "Import glTF to current folder"
    )));

    let imports = make_import_service(project.clone());
    let host = host_foreign_box_with_project(&runtime, project_com, imports);
    let (recorder, ui) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("Import glTF (Replace)...".to_string(), true);
    assert_eq!(
        call_entry(
            &runtime,
            "render_preview_import_entry",
            ui,
            host,
            "/data/model.pol",
        ),
        1
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(
        call,
        UiCall::ButtonEnabled { label, enabled: true, .. }
            if label == "Import glTF (Replace)..."
    )));
}

#[test]
fn import_wizard_replace_entry_point_shows_resolved_target() {
    let root = scratch_dir("replace-entry");
    let pkg_dir = root.join("pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    // Build a real, catalog-mountable `.zip` package with a single
    // `model.pol` entry so `set_replace_target` (called from
    // `open_wizard_for_replace`) has something to resolve.
    {
        use std::io::Write;
        let zip_path = pkg_dir.join("data.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("model.pol", options).unwrap();
        zip.write_all(b"existing pol bytes").unwrap();
        zip.finish().unwrap();
    }

    let (vfs, catalog) = packfs::init_virtual_fs_with_catalog(&pkg_dir, None);
    let project = make_project_service(root.join("base"));
    let imports =
        yaobow_editor::services::ImportService::create(Rc::new(vfs), Rc::new(catalog), project);
    let env = init_env_replace(imports.clone(), "/data/model.pol");

    assert_eq!(imports.mode(), 0);
    assert_eq!(imports.target_format(), 1);
    assert!(imports.add_to_project_enabled());
    assert!(!imports.save_to_file_enabled());

    let (recorder, ui_com) = RecordingUiHost::create();
    render(&env, ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        calls.iter().any(
            |c| matches!(c, UiCall::Text(s) if s.contains("Target:") && s.contains("model.pol"))
        ),
        "expected the resolved replace target to be shown, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "Fixed by replace target: POL")),
        "expected Replace format to be derived and locked, got {calls:?}"
    );
    assert!(
        !calls.iter().any(
            |c| matches!(c, UiCall::TreeLeaf { label, .. } if label == "Replace an existing asset" || label == "Add a new asset")
        ),
        "fixed Replace entry must omit mode choices, got {calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, UiCall::Checkbox { label, .. }
            if label.contains("active project") || label.contains("disk"))),
        "fixed Replace entry must omit output choices, got {calls:?}"
    );
}

#[test]
fn import_wizard_shows_diagnostics_and_success_status_after_run() {
    let root = scratch_dir("run-success");
    let glb = write_glb_fixture(&root, "model.glb");

    let project = make_project_service(root.join("base"));
    let imports = make_import_service(project);

    imports.set_source_path(glb.to_str().unwrap());
    imports.set_target_format(1); // Pol
    imports.set_mode(1); // Add
    imports.set_add_target_package("scene/q01.cpk");
    let out_path = root.join("out.pol");
    imports.set_save_to_file(true);
    imports.set_save_output_path(out_path.to_str().unwrap());

    // Run the conversion through the same `IImportService` handle the
    // wizard will render against (bypassing the file/package pickers,
    // which the recording host can't drive meaningfully).
    assert!(imports.run());
    assert_eq!(imports.status(), 1);

    let env = init_env_shown(imports.clone());

    let (recorder, ui_com) = RecordingUiHost::create();
    render(&env, ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.contains("Status: Success"))),
        "expected the success status line, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.starts_with("Converted size:"))),
        "expected the converted-size line, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.starts_with("Diagnostics:"))),
        "expected the diagnostics-count line, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "Files to import: 1")),
        "expected the planned-file summary, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::ChildWindow { id, .. } if id == "##import_planned_files")),
        "expected the planned-file list, got {calls:?}"
    );
    assert!(
        out_path.exists(),
        "expected the save-to-file output to exist"
    );
}

#[test]
fn import_wizard_preview_staged_asset_button_reflects_staged_preview_path() {
    let root = scratch_dir("preview-staged");
    let glb = write_glb_fixture(&root, "model.glb");

    let project = make_project_service(root.join("base"));
    assert!(project.create_project(root.join("proj").to_str().unwrap()));
    let imports = make_import_service(project);

    imports.set_source_path(glb.to_str().unwrap());
    imports.set_target_format(1); // Pol
    imports.set_mode(1); // Add
    imports.set_add_target_package("scene/q01.cpk");
    imports.set_add_target_directory("q01");
    imports.set_add_to_project(true);
    assert!(imports.run());
    assert_eq!(imports.status(), 1);

    let env = init_env_shown(imports.clone());

    let (recorder, ui_com) = RecordingUiHost::create();
    let picks_len = render(&env, ui_com);
    let calls = recorder.calls.borrow().clone();

    // No live catalog mount for "scene/q01.cpk" in this test, so
    // `staged_preview_vfs_path()` is empty and the button must be
    // absent — live-previewability of a staged change is covered by
    // the Rust unit tests in `import_service.rs`'s own test module.
    let staged = imports.staged_preview_vfs_path();
    assert_eq!(staged, "");
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, UiCall::Button { label, .. } if label == "Preview staged asset")),
        "expected no \"Preview staged asset\" button without a live-previewable staged path, got {calls:?}"
    );
    assert_eq!(picks_len, 0);
}
