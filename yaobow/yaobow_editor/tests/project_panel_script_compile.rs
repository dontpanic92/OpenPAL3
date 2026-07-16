//! Compile + smoke tests for `scripts/project_panel.p7` (the "File"
//! main-menu entry and the "Project Changes" panel).
//!
//! `project_panel.p7` is already exercised indirectly by
//! `scripted_welcome_smoke.rs`'s `welcome_scripts_compile_with_shared_ui_module`
//! test, which loads the full `/yaobow_editor/main.p7` bundle (main.p7
//! -> main_editor.p7 -> project_panel.p7). This file adds dedicated,
//! narrowly-scoped coverage:
//!   - a minimal probe script that imports `project_panel` in
//!     isolation, to pin down compile failures to this module
//!     specifically rather than the whole editor bundle;
//!   - a functional smoke test that actually renders the "File"
//!     menu and the "Project Changes" panel against a real
//!     `ProjectService` and a `RecordingUiHost`, driving a real
//!     create -> stage change through the p7-level entry points (not
//!     just the Rust API directly).
//!
//! Note: `RecordingUiHost::list_clipped` (in `radiance_scripting`)
//! invokes its body exactly once with `list_clipped_index() == -1`
//! rather than simulating one call per row, so per-row Remove/Revert
//! button clicks can't be driven through it here. Row-level behavior
//! (remove/revert semantics, base-hash capture, etc.) is covered by
//! the Rust unit tests in `src/services/project_service.rs` instead;
//! this file only checks that the row renderer runs without panicking
//! and that the summary text (name/dir/tracked-change count) is
//! correct.

use std::cell::RefCell;
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
    IProjectService,
};

// Mirrors `welcome_script_compile.rs`: `radiance_scripting::comdef` only
// contains `services`; `yaobow_editor::comdef` already re-exports the
// same `services` namespace (plus `editor_services`), so taking the
// editor-side glob alone avoids the `services` ambiguous-import
// future-incompat warning — and gives the `ComObject_*!` macros below a
// local `comdef` path to resolve against.
mod comdef {
    pub use yaobow_editor::comdef::*;
}

/// Minimal probe script exercising `project_panel.p7` without pulling
/// in the rest of `main_editor.p7` (which additionally needs a fully
/// wired `vfs()`/`previewers()` host to render its asset tree). Only
/// `host.config()` and `host.project()` are touched by the "File"
/// menu/panel, so a much smaller host stub suffices here.
const PROBE_SRC: &str = r#"
import radiance;
import yaobow_editor.yaobow_editor_services;
import yaobow_editor.project_panel;
import yaobow_editor.main_editor;

pub struct[radiance.IUiLayer, radiance.IDirector] Harness(
    pub host: box<yaobow_editor_services.IEditorHostContext>,
    pub state: box<project_panel.ProjectPanelState>,
) {
    pub fn activate(self: refmut<Self>) -> int { 0 }
    pub fn deactivate(self: refmut<Self>) -> int { 0 }

    pub fn render(self: refmut<Self>, ui: box<radiance.IUiHost>, dt: float) -> int {
        project_panel.render_file_menu(ui, self.host, self.state);
        ui.menu("View", () => {
            project_panel.render_view_menu_item(ui, self.state);
        });
        project_panel.render_project_changes_panel(ui, self.host, self.state);
        ui.text(main_editor.project_status_label(self.host.project()));
        0
    }

    pub fn update(self: refmut<Self>, dt: float) -> ?box<radiance.IDirector> {
        return null;
    }
}

pub fn init(host: box<yaobow_editor_services.IEditorHostContext>) -> box<radiance.IDirector> {
    return box(Harness(host, box(project_panel.make_project_panel_state())));
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

/// `project_panel.p7` in isolation: compiles and resolves its imports
/// against the same module provider used in production.
#[test]
fn project_panel_module_compiles_standalone() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("project_panel.p7 should compile and resolve");
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
        panic!("scene_manager should not be called by the project panel smoke test")
    }
    fn audio(&self) -> ComRc<IAudioService> {
        panic!("audio should not be called by the project panel smoke test")
    }
    fn textures(&self) -> ComRc<ITextureService> {
        panic!("textures should not be called by the project panel smoke test")
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        panic!("vfs should not be called by the project panel smoke test")
    }
    fn input(&self) -> ComRc<IInputService> {
        panic!("input should not be called by the project panel smoke test")
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        GameRegistry::create()
    }
    fn app(&self) -> ComRc<IAppService> {
        panic!("app should not be called by the project panel smoke test")
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
        panic!("previewers should not be called by the project panel smoke test")
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
        panic!("not used by project_panel_script_compile")
    }
    fn render_pending_previews(&self) {}
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_project_service(base_asset_root: std::path::PathBuf) -> ComRc<IProjectService> {
    let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub);
    yaobow_editor::services::ProjectService::create(
        shared::GameType::PAL3,
        base_asset_root,
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(std::path::PathBuf::new())),
        yaobow_editor::services::project_overlay::new_shared_overlay_index(),
        previewers,
    )
    .1
}

/// The project panel probe script doesn't exercise the import wizard,
/// so this is an independent, throwaway `IImportService` — it doesn't
/// need to share state with whatever `IProjectService` `init_env` is
/// given.
fn make_import_service() -> ComRc<IImportService> {
    let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub);
    let (handle, _) = yaobow_editor::services::ProjectService::create(
        shared::GameType::PAL3,
        std::path::PathBuf::new(),
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(std::path::PathBuf::new())),
        yaobow_editor::services::project_overlay::new_shared_overlay_index(),
        previewers,
    );
    yaobow_editor::services::ImportService::create(
        Rc::new(mini_fs::MiniFs::new(false)),
        Rc::new(packfs::AssetCatalog::new(std::path::PathBuf::new())),
        handle,
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

fn init_env(project: ComRc<IProjectService>) -> Env {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(PROBE_SRC)
        .expect("project_panel.p7 probe script should compile");

    let host_ctx = ComRc::<IEditorHostContext>::from_object(TestHostContext {
        config: make_config_service(),
        project,
        imports: make_import_service(),
    });
    let host_id = runtime.intern(host_ctx);
    let host = runtime
        .foreign_box(
            "yaobow_editor.comdef.editor_services.IEditorHostContext",
            host_id,
        )
        .expect("host foreign box");
    let state = runtime
        .call_returning_data("init", vec![host])
        .expect("probe init should run");
    let handle = runtime.root(state);
    Env { runtime, handle }
}

fn render(env: &Env, ui_com: ComRc<IUiHost>) {
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("harness director should be rooted");
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box("radiance.comdef.IUiHost", ui_com_id)
        .expect("ui_host foreign box");
    env.runtime
        .call_method_void(director, "render", vec![ui_box, Data::Float(0.0)])
        .expect("harness render should run");
}

#[test]
fn file_and_view_menus_render_with_no_active_project() {
    let project = make_project_service(std::path::PathBuf::from("/base"));
    let env = init_env(project);
    let (recorder, ui_com) = RecordingUiHost::create();
    // Simulate clicking "Show Project Changes" so the summary panel (and its
    // "No active project" message) actually renders this frame.
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("Show Project Changes".to_string(), true);

    render(&env, ui_com);

    // The harness calls `render_file_menu` directly (production
    // wraps it in `ui.main_menu_bar(...)` from `main_editor.p7`), so
    // there's no `MainMenuBar` call to expect here — only the `Menu`
    // itself and its items.
    let calls = recorder.calls.borrow().clone();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Menu { label } if label == "File")),
        "expected a \"File\" Menu call, got {calls:?}"
    );
    let expected_items = [
        "New Project...",
        "Open Project...",
        "Save Project",
        "Close Project",
        "Publish .yapatch...",
    ];
    for item in expected_items {
        assert!(
            calls
                .iter()
                .any(|c| matches!(c, UiCall::MenuItem { label, .. } if label == item)),
            "expected a \"{item}\" MenuItem call, got {calls:?}"
        );
    }
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Menu { label } if label == "View")),
        "expected a \"View\" Menu call, got {calls:?}"
    );
    assert!(
        calls.iter().any(
            |c| matches!(c, UiCall::MenuItem { label, .. } if label == "Show Project Changes")
        ),
        "expected the View toggle, got {calls:?}"
    );
    let has_no_project_text = calls
        .iter()
        .any(|c| matches!(c, UiCall::Text(s) if s.contains("No active project")));
    assert!(
        has_no_project_text,
        "expected the changes panel to report no active project when toggled, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "项目: 未打开")),
        "expected the menu-bar no-project status label, got {calls:?}"
    );
}

#[test]
fn project_changes_panel_shows_active_project_summary_and_tracked_change_count() {
    let root = scratch_dir("panel-summary");
    let project = make_project_service(root.join("base"));
    assert!(project.create_project(root.join("proj").to_str().unwrap()));

    let source_file = root.join("source.txt");
    std::fs::write(&source_file, b"hello").unwrap();
    assert!(project.stage_payload_file(
        "scene/q01.cpk",
        "q01/q01.scn",
        source_file.to_str().unwrap(),
        "",
        "",
    ));
    assert_eq!(project.change_count(), 1);

    let env = init_env(project.clone());
    let (recorder, ui_com) = RecordingUiHost::create();
    // Panel visibility is toggled by clicking "Show Project Changes"
    // menu item; simulate that click so `render_project_changes_panel`
    // actually opens the window this frame.
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("Show Project Changes".to_string(), true);

    render(&env, ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        calls.iter().any(
            |c| matches!(c, UiCall::WindowCenteredClosable { title, .. } if title.starts_with("Project Changes"))
        ),
        "expected the changes panel window once toggled on, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.contains("proj"))),
        "expected the active project name to be shown, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.contains("Tracked changes: 1"))),
        "expected the tracked-change count to be shown, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "项目: proj *")),
        "expected the dirty active-project status label, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::ListClipped { count } if *count == 1)),
        "expected list_clipped to be called with the tracked-change count, got {calls:?}"
    );

    // Removing directly via the service (bypassing the row button,
    // whose id depends on `list_clipped_index()` — see module docs)
    // should be reflected the next time the panel renders. The panel's
    // `show_changes_panel` flag already persisted as `true` from the
    // first render above, so this second render must NOT re-click the
    // "Project Changes" menu item (that would toggle it back off).
    assert!(project.remove_change(0));
    let (recorder2, ui_com2) = RecordingUiHost::create();
    render(&env, ui_com2);
    let calls2 = recorder2.calls.borrow().clone();
    assert!(
        calls2
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s.contains("Tracked changes: 0"))),
        "expected the tracked-change count to drop to 0 after removal, got {calls2:?}"
    );
}

#[test]
fn project_changes_title_bar_close_hides_the_panel() {
    let project = make_project_service(std::path::PathBuf::from("/base"));
    let env = init_env(project);

    let (open_recorder, open_ui) = RecordingUiHost::create();
    open_recorder
        .menu_item_results
        .borrow_mut()
        .insert("Show Project Changes".to_string(), true);
    render(&env, open_ui);

    let (close_recorder, close_ui) = RecordingUiHost::create();
    close_recorder
        .window_close_results
        .borrow_mut()
        .insert("Project Changes###project_changes".to_string(), true);
    render(&env, close_ui);
    assert!(
        close_recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::WindowCenteredClosable { .. }))
    );

    let (after_recorder, after_ui) = RecordingUiHost::create();
    render(&env, after_ui);
    assert!(
        !after_recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::WindowCenteredClosable { .. }))
    );
}
