//! Verifies welcome.p7 and settings.p7 parse and load against the same
//! module-provider as production.
use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance::comdef::ISceneManager;
use radiance_scripting::comdef::services::{
    IAudioService, ICommandBus, IConfigService, IGameRegistry, IHostContext, IHostContextImpl,
    IInputService, ITextureService, IVfsService,
};
use radiance_scripting::services::GameRegistry;
use radiance_scripting::ui_walker::{kinds, owned};

const WELCOME_SRC: &str = include_str!("../../../yaobow/yaobow_editor/scripts/welcome.p7");
const SETTINGS_SRC: &str = include_str!("../../../yaobow/yaobow_editor/scripts/settings.p7");

mod comdef {
    pub use radiance_scripting::comdef::*;
}

struct TestHostContext;

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

    fn commands(&self) -> ComRc<ICommandBus> {
        panic!("commands should not be called while initializing welcome/settings scripts")
    }

    fn config(&self) -> ComRc<IConfigService> {
        panic!("config should not be called while initializing welcome/settings scripts")
    }
}

#[test]
fn welcome_script_compiles() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(WELCOME_SRC)
        .expect("welcome.p7 should compile");
}

#[test]
fn settings_script_compiles() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(SETTINGS_SRC)
        .expect("settings.p7 should compile");
}

#[test]
fn welcome_script_init_loads_with_imported_bindings() {
    init_script(WELCOME_SRC).expect("welcome.p7 init should load");
}

#[test]
fn welcome_script_renders_three_column_wide_layout() {
    let mut runtime = init_runtime(WELCOME_SRC).expect("welcome.p7 init should load");
    let state = runtime
        .state_clone()
        .expect("welcome state should be stored");
    let node = runtime
        .call_returning_data("render", vec![state, Data::Float(0.0)])
        .expect("welcome.p7 render should return a UiNode");
    let owned = runtime
        .with_ctx(|ctx| owned::resolve(ctx, &node))
        .expect("welcome UiNode should resolve");

    assert_eq!(owned.kind, kinds::WINDOW_CENTERED);
    assert_eq!(owned.w, 1180.0);
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
fn settings_script_init_loads_with_imported_bindings() {
    init_script(SETTINGS_SRC).expect("settings.p7 init should load");
}

fn init_script(source: &str) -> Result<(), crosscom_protosept::HostError> {
    init_runtime(source).map(|_| ())
}

fn init_runtime(
    source: &str,
) -> Result<radiance_scripting::ScriptRuntime, crosscom_protosept::HostError> {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime.load_source(source)?;
    let host_ctx = ComRc::<IHostContext>::from_object(TestHostContext);
    let host_id = runtime.intern(host_ctx);
    let host = runtime.foreign_box("radiance_scripting.comdef.services.IHostContext", host_id)?;
    let state = runtime.call_returning_data("init", vec![host])?;
    runtime.store_state(state);
    Ok(runtime)
}
