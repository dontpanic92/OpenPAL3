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
use radiance_scripting::services::GameRegistry;
use radiance_scripting::ui_walker::{kinds, owned};
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
fn welcome_script_renders_three_column_wide_layout() {
    let env = init_runtime(MAIN_P7).expect("editor script init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let node = env
        .runtime
        .call_method_returning_data(director, "render", vec![Data::Float(0.0)])
        .expect("welcome.p7 render should return a UiNode");
    let owned = env
        .runtime
        .with_ctx(|ctx| owned::resolve(ctx, &node))
        .expect("welcome UiNode should resolve");

    assert_eq!(owned.kind, kinds::WINDOW_CENTERED);
    assert_eq!(owned.w, 720.0);
    assert_eq!(owned.h, 480.0);

    let table = owned
        .children
        .iter()
        .find(|child| child.kind == kinds::TABLE)
        .expect("welcome layout should include a game table");
    assert_eq!(table.i1, 3);
    assert_eq!(table.children.len(), 15);

    let buttons = table
        .children
        .iter()
        .filter(|child| child.kind == kinds::BUTTON)
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 10);
    assert!(buttons.iter().all(|button| button.w == -1.0));
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
    assert_eq!(result, Data::Array(std::rc::Rc::new(Vec::new())));
}

#[test]
fn welcome_script_settings_command_returns_script_director() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let result = env
        .runtime
        .call_method_returning_data(director, "dispatch", vec![Data::Int(9001)])
        .expect("welcome.p7 dispatch should return");

    match result {
        Data::Array(values) => assert_eq!(values.len(), 1),
        other => panic!("expected one script director transition, got {other:?}"),
    }
}

#[test]
fn welcome_script_unknown_command_is_a_no_op() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let result = env
        .runtime
        .call_method_returning_data(director, "dispatch", vec![Data::Int(9999)])
        .expect("welcome.p7 dispatch should return");
    assert_eq!(result, Data::Array(std::rc::Rc::new(Vec::new())));
}

#[test]
fn welcome_script_game_command_with_configured_path_calls_open_game() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    env.config_paths
        .borrow_mut()
        .insert(0, "/tmp/openpal3".to_string());
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let result = env
        .runtime
        .call_method_returning_data(director, "dispatch", vec![Data::Int(1000)])
        .expect("welcome.p7 dispatch should return");

    match result {
        Data::Array(values) => assert_eq!(values.len(), 1),
        other => panic!("expected one wrapped host director, got {other:?}"),
    }
    assert_eq!(*env.open_calls.borrow(), vec![0]);
}

#[test]
fn welcome_script_render_update_survives_repeated_frames() {
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    for _ in 0..200 {
        let director = env
            .runtime
            .deref_handle(env.handle)
            .expect("welcome director should be rooted");
        let node = env
            .runtime
            .call_method_returning_data(director.clone(), "render", vec![Data::Float(0.0)])
            .expect("welcome.p7 render should return a UiNode");
        env.runtime
            .with_ctx(|ctx| owned::resolve(ctx, &node))
            .expect("welcome UiNode should resolve");
        let result = env
            .runtime
            .call_method_returning_data(director, "update", vec![Data::Float(0.0)])
            .expect("welcome.p7 update should return");
        assert_eq!(result, Data::Array(std::rc::Rc::new(Vec::new())));
    }
}

#[test]
fn host_director_returned_by_open_game_renders_and_updates() {
    // Mirrors the editor flow that the user reported failing: pick a game
    // with a configured asset path, take the wrapped HostDirector returned
    // by welcome.dispatch, and drive it through render/update.
    let env = init_runtime(MAIN_P7).expect("welcome.p7 init should load");
    env.config_paths
        .borrow_mut()
        .insert(0, "/tmp/openpal3".to_string());
    let director = env
        .runtime
        .deref_handle(env.handle)
        .expect("welcome director should be rooted");
    let result = env
        .runtime
        .call_method_returning_data(director, "dispatch", vec![Data::Int(1000)])
        .expect("welcome.p7 dispatch should return");

    let next_director = match result {
        Data::Array(values) => {
            assert_eq!(values.len(), 1, "expected one wrapped host director");
            values[0].clone()
        }
        other => panic!("expected one wrapped host director, got {other:?}"),
    };

    // Drive the HostDirector through render+update — this is what
    // ScriptedDirector::update would do every frame on the active
    // director after the game-pick transition.
    for _ in 0..3 {
        let node = env
            .runtime
            .call_method_returning_data(next_director.clone(), "render", vec![Data::Float(0.0)])
            .expect("HostDirector.render should return a UiNode");
        env.runtime
            .with_ctx(|ctx| owned::resolve(ctx, &node))
            .expect("HostDirector UiNode should resolve");
        let updated = env
            .runtime
            .call_method_returning_data(next_director.clone(), "update", vec![Data::Float(0.016)])
            .expect("HostDirector.update should return");
        assert_eq!(updated, Data::Array(std::rc::Rc::new(Vec::new())));
    }
}
