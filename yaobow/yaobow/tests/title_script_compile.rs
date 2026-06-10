//! Verifies the yaobow title-page p7 scripts parse and compile
//! against the same module-provider production uses. The first test
//! is a pure compile check; the second drives the script through
//! `init` with a stub host context to catch issues that only surface
//! when the runtime materialises the foreign types referenced by
//! `title.p7`'s field declarations.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::{ComRc, IObjectArray};
use radiance::comdef::IImmediateDirector;
use radiance::comdef::{IDirector, ISceneManager};
use radiance_scripting::comdef::services::{
    IAppService, IAppServiceImpl, IAudioService, IAudioServiceImpl, IConfigService, IGameRegistry,
    IGifAnimation, IHostContext, IHostContextImpl, IInputService, IRandomService, ITexture,
    ITextureService, ITextureServiceImpl, IVfsService,
};
use radiance_scripting::services::{GameRegistry, RandomService};
use radiance_scripting::{
    RuntimeAccess, RuntimeHandle, ScriptHost, register_immediate_director_proto, with_services,
};
use yaobow_lib::comdef::yaobow_services::IYaobowScriptApp;
use yaobow_lib::script_bridges::yaobow_services::wrap_yaobow_script_app;
use yaobow_lib::script_source::install_script_assets;

/// Helper: build a fresh `ScriptHost` with the dedicated script
/// `AssetManager` already installed, then load `/yaobow/app.p7` via
/// the VFS-backed module provider.
fn fresh_runtime_with_yaobow_loaded() -> Rc<ScriptHost> {
    let runtime = ScriptHost::new();
    runtime.set_script_assets(install_script_assets());
    runtime
        .load_source_from_path("/yaobow/app.p7")
        .expect("yaobow app script should compile");
    runtime
}

// `ComObject_*!` macros expand `use crate as radiance_scripting` and
// then reach into `crate::comdef::*` to find the impl traits and
// interface symbols. Re-export both `radiance_scripting`'s and
// `yaobow_lib`'s comdef modules — but disambiguate `services`
// explicitly because both crates publish it.
mod comdef {
    pub use radiance_scripting::comdef::services;
    pub use yaobow_lib::comdef::yaobow_services;
}

struct RecordingAppService {
    open_calls: Rc<RefCell<Vec<i32>>>,
}

radiance_scripting::ComObject_AppService!(crate::RecordingAppService);

impl IAppServiceImpl for RecordingAppService {
    fn open_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        self.open_calls.borrow_mut().push(ordinal);
        None
    }

    fn exit(&self) {}

    fn set_title(&self, _title: &str) {}
}

struct StubAudioService;

radiance_scripting::ComObject_AudioService!(crate::StubAudioService);

impl IAudioServiceImpl for StubAudioService {
    fn load(
        &self,
        _vfs_path: &str,
        _codec: i32,
    ) -> Option<ComRc<radiance_scripting::comdef::services::IAudioSource>> {
        None
    }
}

struct StubTextureService;

radiance_scripting::ComObject_TextureService!(crate::StubTextureService);

impl ITextureServiceImpl for StubTextureService {
    fn load_png(&self, _vfs_path: &str) -> Option<ComRc<ITexture>> {
        None
    }

    fn load_gif_frames(&self, _vfs_path: &str) -> Option<ComRc<IObjectArray>> {
        None
    }

    fn load_gif_animation(&self, _vfs_path: &str) -> Option<ComRc<IGifAnimation>> {
        None
    }
}

struct TestHostContext {
    app: ComRc<IAppService>,
    config: ComRc<IConfigService>,
}

fn host_runtime_handle(host: &Rc<ScriptHost>) -> RuntimeHandle {
    let mut out = None;
    <ScriptHost as RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = with_services(|s| s.runtime_handle()).expect("with_services inside scope");
        out = Some(h);
    });
    out.expect("RuntimeAccess::with_ctx ran body")
}

yaobow_lib::ComObject_YaobowAppContext!(crate::TestHostContext);

impl IHostContextImpl for TestHostContext {
    fn scene_manager(&self) -> ComRc<ISceneManager> {
        panic!("scene_manager should not be called during title script init")
    }
    fn audio(&self) -> ComRc<IAudioService> {
        ComRc::<IAudioService>::from_object(StubAudioService)
    }
    fn textures(&self) -> ComRc<ITextureService> {
        ComRc::<ITextureService>::from_object(StubTextureService)
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        panic!("vfs should not be called during title script init")
    }
    fn input(&self) -> ComRc<IInputService> {
        panic!("input should not be called during title script init")
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        GameRegistry::create()
    }
    fn app(&self) -> ComRc<IAppService> {
        self.app.clone()
    }
    fn random(&self) -> ComRc<IRandomService> {
        RandomService::create()
    }
    fn config(&self) -> ComRc<IConfigService> {
        self.config.clone()
    }
}

fn make_test_config() -> ComRc<IConfigService> {
    let cfg = Rc::new(RefCell::new(shared::config::YaobowConfig::default()));
    shared::config_service::ConfigService::create(cfg)
}

#[test]
fn title_script_compiles() {
    fresh_runtime_with_yaobow_loaded();
}

#[test]
fn title_script_init_loads_with_imported_bindings() {
    register_immediate_director_proto();
    let open_calls = Rc::new(RefCell::new(Vec::new()));
    let app = ComRc::<IAppService>::from_object(RecordingAppService {
        open_calls: open_calls.clone(),
    });
    let app_ctx = ComRc::<IHostContext>::from_object(TestHostContext {
        app,
        config: make_test_config(),
    });

    let runtime = fresh_runtime_with_yaobow_loaded();
    let app_ctx_id = runtime.intern(app_ctx);
    let app_ctx_box = runtime
        .foreign_box(
            "radiance_scripting.comdef.services.IHostContext",
            app_ctx_id,
        )
        .expect("IHostContext foreign box must construct");
    let app_data = runtime
        .call_returning_data("init", vec![app_ctx_box])
        .expect("yaobow app init should succeed");
    // Reverse-wrap the app root into a real `ComRc<IYaobowScriptApp>`
    // (the production path) and call its factory method through the COM
    // vtable — no manual `call_method_returning_data` name-dispatch.
    let handle = host_runtime_handle(&runtime);
    let factory: ComRc<IYaobowScriptApp> =
        wrap_yaobow_script_app(&handle, app_data).expect("wrap_yaobow_script_app should succeed");
    let im: ComRc<IImmediateDirector> = factory.make_title_director();
    let director: ComRc<IDirector> = im
        .query_interface::<IDirector>()
        .expect("title director should expose IDirector via fat CCW");
    director.activate();
    assert!(director.update(0.016).is_none());
    drop(director);
    drop(im);
    drop(factory);
}

#[test]
fn app_script_creates_title_then_pal4_debug_in_one_runtime() {
    let open_calls = Rc::new(RefCell::new(Vec::new()));
    let app = ComRc::<IAppService>::from_object(RecordingAppService {
        open_calls: open_calls.clone(),
    });
    let app_ctx = ComRc::<IHostContext>::from_object(TestHostContext {
        app,
        config: make_test_config(),
    });

    let runtime = fresh_runtime_with_yaobow_loaded();
    let app_ctx_id = runtime.intern(app_ctx);
    let app_ctx_box = runtime
        .foreign_box(
            "radiance_scripting.comdef.services.IHostContext",
            app_ctx_id,
        )
        .expect("IHostContext foreign box must construct");
    let app_data = runtime
        .call_returning_data("init", vec![app_ctx_box])
        .expect("yaobow app init should succeed");
    // One reverse-wrapped factory drives both make_* calls through the
    // COM vtable. The `IPal4DebugContext` arg is passed as a plain
    // `ComRc` — the proto-CCW marshals it into a foreign box per the
    // registered ProtoSpec, so no manual intern/foreign_box is needed.
    // Register the shared PAL4 factory proto so the fat CCW exposes a QI
    // slot for it (the app struct conforms to it in addition to
    // `IYaobowScriptApp`).
    shared::script_bridges::openpal4::register_pal4_script_factory_proto();
    let handle = host_runtime_handle(&runtime);
    let factory: ComRc<IYaobowScriptApp> =
        wrap_yaobow_script_app(&handle, app_data).expect("wrap_yaobow_script_app should succeed");

    let title_director: ComRc<IImmediateDirector> = factory.make_title_director();
    drop(title_director);

    let pal4_factory = factory
        .query_interface::<shared::openpal4::comdef::IPal4ScriptFactory>()
        .expect("app struct must conform to IPal4ScriptFactory");
    let session = shared::openpal4::pal4_debug::create_debug_session();
    let _overlay = pal4_factory.make_pal4_debug_overlay(session.context);
}
