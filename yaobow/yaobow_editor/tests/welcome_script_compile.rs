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
    IGameRegistry, IHostContext, IHostContextImpl, IInputService, ITextureService, IVfsService,
};
use radiance_scripting::services::GameRegistry;
use radiance_scripting::ui_walker::{kinds, owned};

const WELCOME_SRC: &str = include_str!("../scripts/welcome.p7");

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
}

radiance_scripting::ComObject_HostContext!(crate::TestHostContext);

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

    let runtime = radiance_scripting::ScriptHost::new();
    runtime.load_source(source)?;
    let host_ctx = ComRc::<IHostContext>::from_object(TestHostContext { app, config });
    let host_id = runtime.intern(host_ctx);
    let host = runtime.foreign_box("radiance_scripting.comdef.services.IHostContext", host_id)?;
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
    runtime
        .load_source(WELCOME_SRC)
        .expect("welcome.p7 should compile");
}

#[test]
fn welcome_script_init_loads_with_imported_bindings() {
    init_script(WELCOME_SRC).expect("welcome.p7 init should load");
}

#[test]
fn welcome_script_renders_three_column_wide_layout() {
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
    let env = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
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
