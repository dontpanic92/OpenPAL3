//! Verifies welcome.p7 parses and loads against the same
//! module-provider as production, and exercises the typed host services
//! (`IAppService`, `IConfigService`) the welcome flow depends on.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance::comdef::{IDirector, IDirectorImpl, ISceneManager};
use radiance_scripting::comdef::services::{
    IAppService, IAppServiceImpl, IAudioService, IConfigService, IConfigServiceImpl,
    IGameRegistry, IHostContextImpl, IInputService, ITextureService, IVfsService,
};
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::services::GameRegistry;
use yaobow_editor::comdef::editor_services::{
    IEditorHostContext, IEditorHostContextImpl, IPreviewerHub, IPreviewerHubImpl,
};
use yaobow_editor::editor_bindings::EDITOR_SERVICES_P7;
use yaobow_editor::script_source::{MAIN_P7, register_editor_modules};

mod comdef {
    pub use radiance_scripting::comdef::*;
    pub use yaobow_editor::comdef::*;
}

struct StubDirector;
radiance_scripting::ComObject_ScriptedDirector!(crate::StubDirector);
impl IDirectorImpl for StubDirector {
    fn activate(&self) {}
    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        None
    }
}

struct RecordingAppService {
    open_calls: Rc<RefCell<Vec<i32>>>,
}

radiance_scripting::ComObject_AppService!(crate::RecordingAppService);

impl IAppServiceImpl for RecordingAppService {
    fn open_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        self.open_calls.borrow_mut().push(ordinal);
        Some(ComRc::from_object(StubDirector))
    }
}

struct StubConfigService {
    paths: Rc<RefCell<HashMap<i32, String>>>,
    last: RefCell<String>,
}

yaobow_editor::ComObject_ConfigService!(crate::StubConfigService);

impl IConfigServiceImpl for StubConfigService {
    fn get_asset_path(&self, game: i32) -> &str {
        let value = self
            .paths
            .borrow()
            .get(&game)
            .cloned()
            .unwrap_or_default();
        *self.last.borrow_mut() = value;
        unsafe { (*self.last.as_ptr()).as_str() }
    }
    fn set_asset_path(&self, game: i32, path: &str) {
        self.paths.borrow_mut().insert(game, path.to_string());
    }
    fn save(&self) -> bool {
        true
    }
    fn reload(&self) {}
    fn pick_folder(&self, _initial: &str) -> &str {
        ""
    }
}

struct TestHostContext {
    app: ComRc<IAppService>,
    config: ComRc<IConfigService>,
    previewers: ComRc<IPreviewerHub>,
}

yaobow_editor::ComObject_EditorHostContext!(crate::TestHostContext);

impl IHostContextImpl for TestHostContext {
    fn scene_manager(&self) -> ComRc<ISceneManager> {
        panic!("scene_manager should not be called while initializing welcome/settings scripts")
    }
    fn audio(&self) -> ComRc<IAudioService> {
        panic!("audio should not be called while initializing welcome/settings scripts")
    }
    fn textures(&self) -> ComRc<ITextureService> {
        panic!("textures should not be called while initializing welcome/settings scripts")
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        panic!("vfs should not be called while initializing welcome/settings scripts")
    }
    fn input(&self) -> ComRc<IInputService> {
        panic!("input should not be called while initializing welcome/settings scripts")
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        GameRegistry::create()
    }
    fn app(&self) -> ComRc<IAppService> {
        self.app.clone()
    }
    fn config(&self) -> ComRc<IConfigService> {
        self.config.clone()
    }
}

impl IEditorHostContextImpl for TestHostContext {
    fn previewers(&self) -> ComRc<IPreviewerHub> {
        self.previewers.clone()
    }
    fn new_render_target(
        &self,
        _w: i32,
        _h: i32,
    ) -> ComRc<radiance_scripting::comdef::services::IRenderTarget> {
        panic!("not used by welcome_script_compile")
    }
    fn render_pending_previews(&self) {
        // no-op in the script-compile smoke test.
    }
}

struct StubPreviewerHub {
    last: std::cell::RefCell<String>,
}
yaobow_editor::ComObject_PreviewerHub!(crate::StubPreviewerHub);
impl IPreviewerHubImpl for StubPreviewerHub {
    fn classify(&self, _vfs_path: &str) -> i32 {
        0
    }
    fn open_text(&self, _vfs_path: &str) -> &str {
        *self.last.borrow_mut() = String::new();
        unsafe { (*self.last.as_ptr()).as_str() }
    }
    fn dump_structured(&self, _vfs_path: &str) -> &str {
        *self.last.borrow_mut() = String::new();
        unsafe { (*self.last.as_ptr()).as_str() }
    }
    fn open_image(&self, _vfs_path: &str) -> Option<ComRc<yaobow_editor::comdef::editor_services::IImageHandle>> {
        None
    }
    fn open_audio(&self, _vfs_path: &str) -> Option<ComRc<yaobow_editor::comdef::editor_services::IAudioHandle>> {
        None
    }
    fn open_video(&self, _vfs_path: &str) -> Option<ComRc<yaobow_editor::comdef::editor_services::IVideoHandle>> {
        None
    }
    fn open_model(&self, _vfs_path: &str) -> Option<ComRc<yaobow_editor::comdef::editor_services::IModelHandle>> {
        None
    }
}

struct TestEnv {
    runtime: Rc<radiance_scripting::ScriptHost>,
    handle: radiance_scripting::ScriptDirectorHandle,
    open_calls: Rc<RefCell<Vec<i32>>>,
    config_paths: Rc<RefCell<HashMap<i32, String>>>,
}

fn init_runtime(source: &str) -> Result<TestEnv, crosscom_protosept::HostError> {
    let open_calls = Rc::new(RefCell::new(Vec::new()));
    let app = ComRc::<IAppService>::from_object(RecordingAppService {
        open_calls: open_calls.clone(),
    });
    let config_paths = Rc::new(RefCell::new(HashMap::new()));
    let config = ComRc::<IConfigService>::from_object(StubConfigService {
        paths: config_paths.clone(),
        last: RefCell::new(String::new()),
    });
    let previewers = ComRc::<IPreviewerHub>::from_object(StubPreviewerHub {
        last: std::cell::RefCell::new(String::new()),
    });

    let runtime = radiance_scripting::ScriptHost::new();
    runtime.add_binding("yaobow_editor_services", EDITOR_SERVICES_P7);
    register_editor_modules(&runtime);
    runtime.load_source(source)?;
    let host_ctx = ComRc::<IEditorHostContext>::from_object(TestHostContext {
        app,
        config,
        previewers,
    });
    let host_id = runtime.intern(host_ctx);
    let host = runtime.foreign_box(
        "yaobow_editor.comdef.editor_services.IEditorHostContext",
        host_id,
    )?;
    let state = runtime.call_returning_data("init", vec![host])?;
    let handle = runtime.root(state);
    Ok(TestEnv {
        runtime,
        handle,
        open_calls,
        config_paths,
    })
}

fn init_script(source: &str) -> Result<(), crosscom_protosept::HostError> {
    init_runtime(source).map(|_| ())
}

#[test]
fn welcome_script_compiles() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.add_binding("yaobow_editor_services", EDITOR_SERVICES_P7);
    register_editor_modules(&runtime);
    runtime
        .load_source(MAIN_P7)
        .expect("editor script should compile");
}

#[test]
fn welcome_script_init_loads_with_imported_bindings() {
    init_script(MAIN_P7).expect("editor script init should load");
}

#[test]
fn welcome_script_render_im_emits_window_centered_with_game_table() {
    let env = init_runtime(MAIN_P7).expect("editor script init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let (recorder, ui_com) = RecordingUiHost::create();
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box(
            "radiance_scripting.comdef.immediate_director.IUiHost",
            ui_com_id,
        )
        .expect("ui_host foreign box");

    env.runtime
        .call_method_void(
            director,
            "render_im",
            vec![ui_box, Data::Float(0.0)],
        )
        .expect("welcome.p7 render_im should run");

    let calls = recorder.calls.borrow().clone();
    // Outer window centered + body + game table opened with 3 columns.
    assert!(
        matches!(
            calls.first(),
            Some(UiCall::WindowCentered { title, w, h })
                if title == "妖弓编辑器" && *w == 720.0 && *h == 480.0
        ),
        "expected WindowCentered at start, got: {:?}",
        calls.first()
    );
    let table = calls
        .iter()
        .find(|c| matches!(c, UiCall::Table { id, .. } if id == "welcome.games"))
        .expect("welcome render_im should emit a Table");
    if let UiCall::Table { cols, .. } = table {
        assert_eq!(*cols, 3, "welcome game table should be 3 columns wide");
    }
    let button_count = calls
        .iter()
        .filter(|c| matches!(c, UiCall::Button { .. }))
        .count();
    // 10 game buttons + 1 "设置" button.
    assert_eq!(
        button_count, 11,
        "welcome should emit 10 game buttons + 1 settings button, got {button_count}: {calls:?}"
    );
}

#[test]
fn welcome_script_update_returns_empty_transition_list() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let result = env
        .runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
        .expect("welcome.p7 update should return");
    assert_eq!(result, Data::Null);
}

#[test]
fn welcome_script_settings_button_routes_to_settings_director() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("设置".to_string(), true);
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box(
            "radiance_scripting.comdef.immediate_director.IUiHost",
            ui_com_id,
        )
        .expect("ui_host foreign box");

    env.runtime
        .call_method_void(
            director.clone(),
            "render_im",
            vec![ui_box, Data::Float(0.0)],
        )
        .expect("render_im should run with the simulated click");

    let next = env
        .runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
        .expect("update should return after a settings click");

    match next {
        Data::Some(_) | Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {}
        other => panic!("expected one transition, got {other:?}"),
    }
}

#[test]
fn welcome_script_no_click_yields_no_transition() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let (_recorder, ui_com) = RecordingUiHost::create();
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box(
            "radiance_scripting.comdef.immediate_director.IUiHost",
            ui_com_id,
        )
        .expect("ui_host foreign box");

    env.runtime
        .call_method_void(
            director.clone(),
            "render_im",
            vec![ui_box, Data::Float(0.0)],
        )
        .expect("render_im should run");
    let result = env
        .runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
        .expect("update should return");
    assert_eq!(result, Data::Null);
}

#[test]
fn welcome_script_game_button_with_configured_path_calls_open_game() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    env.config_paths
        .borrow_mut()
        .insert(0, "/tmp/openpal3".to_string());
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("运行《仙剑奇侠传三》编辑器".to_string(), true);
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box(
            "radiance_scripting.comdef.immediate_director.IUiHost",
            ui_com_id,
        )
        .expect("ui_host foreign box");

    env.runtime
        .call_method_void(
            director.clone(),
            "render_im",
            vec![ui_box, Data::Float(0.0)],
        )
        .expect("render_im should run with the simulated click");

    let next = env
        .runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
        .expect("update should return after a game click");

    match next {
        Data::Some(_) | Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {}
        other => panic!("expected one wrapped host director, got {other:?}"),
    }
    assert_eq!(*env.open_calls.borrow(), vec![0]);
}

#[test]
fn welcome_script_render_im_update_survives_repeated_frames() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let (_recorder, ui_com) = RecordingUiHost::create();
    let ui_com_id = env.runtime.intern(ui_com);
    for _ in 0..200 {
        let director = env
            .runtime
            .deref_handle(env.handle)
            .expect("welcome director should be rooted");
        let ui_box = env
            .runtime
            .foreign_box(
                "radiance_scripting.comdef.immediate_director.IUiHost",
                ui_com_id,
            )
            .expect("ui_host foreign box");
        env.runtime
            .call_method_void(
                director.clone(),
                "render_im",
                vec![ui_box, Data::Float(0.0)],
            )
            .expect("welcome.p7 render_im should run");
        let result = env
            .runtime
            .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
            .expect("welcome.p7 update should return");
        assert_eq!(result, Data::Null);
    }
}

#[test]
fn welcome_script_game_button_with_configured_path_returns_open_game_director() {
    // Phase 6: welcome.update on a game-pick with a configured path
    // returns the `ComRc<IDirector>` that `open_game` produced. The
    // engine then makes that the active director and the pump fires
    // its `render_im` via QI (when it conforms to IImmediateDirector).
    // Here open_game's stub returns a plain `StubDirector`, so we
    // just verify the transition value is the foreign box itself.
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    env.config_paths
        .borrow_mut()
        .insert(0, "/tmp/openpal3".to_string());
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("运行《仙剑奇侠传三》编辑器".to_string(), true);
    let ui_com_id = env.runtime.intern(ui_com);
    let ui_box = env
        .runtime
        .foreign_box(
            "radiance_scripting.comdef.immediate_director.IUiHost",
            ui_com_id,
        )
        .expect("ui_host foreign box");
    env.runtime
        .call_method_void(
            director.clone(),
            "render_im",
            vec![ui_box, Data::Float(0.0)],
        )
        .expect("render_im should run with the simulated click");
    let result = env
        .runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
        .expect("welcome.p7 update should return");

    match result {
        Data::Some(_) | Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {}
        other => panic!("expected a transition box, got {other:?}"),
    }
    assert_eq!(*env.open_calls.borrow(), vec![0]);
}
