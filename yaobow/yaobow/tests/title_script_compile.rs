//! Verifies the yaobow title-page p7 scripts parse and compile
//! against the same module-provider production uses. The first test
//! is a pure compile check; the second drives the script through
//! `init` with a stub host context to catch issues that only surface
//! when the runtime materialises the foreign types referenced by
//! `title.p7`'s field declarations.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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
    OwnedScriptModule, OwnedScriptPackage, RuntimeAccess, RuntimeHandle, ScriptHost,
    register_immediate_director_proto, with_services, wrap_director,
};
use yaobow_lib::script_source::package;

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
    let runtime = ScriptHost::new();
    package()
        .ensure_loaded(&runtime, "init")
        .expect("yaobow app script should compile");
}

#[test]
fn script_package_rejects_duplicate_modules() {
    fn module(name: &str, src: &str) -> OwnedScriptModule {
        OwnedScriptModule::new(name.to_string(), src.to_string())
    }
    let package = OwnedScriptPackage {
        root_name: Some("app".to_string()),
        root_source: Some(Arc::<str>::from("pub fn init() -> int { 0 }")),
        idl_bindings: vec![],
        modules: vec![
            module("dup", "pub fn a() -> int { 1 }"),
            module("dup", "pub fn b() -> int { 2 }"),
        ],
    };

    let err = package
        .validate()
        .expect_err("duplicate module should fail validation");
    assert!(err.contains("duplicate"));
    assert!(err.contains("dup"));
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

    let runtime = ScriptHost::new();
    package()
        .ensure_loaded(&runtime, "init")
        .expect("yaobow app script should compile");
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
    let app_handle = runtime.root(app_data);
    let app_data = runtime
        .deref_handle(app_handle)
        .expect("yaobow app root should stay live");
    let director_data = runtime
        .call_method_returning_data(app_data, "make_title_director", Vec::new())
        .expect("yaobow title director creation should succeed");
    let handle = host_runtime_handle(&runtime);
    let director: ComRc<IDirector> =
        wrap_director(&handle, director_data).expect("wrap_director should succeed");
    let im: ComRc<IImmediateDirector> = director
        .query_interface::<IImmediateDirector>()
        .expect("title director should expose IImmediateDirector via fat CCW");
    director.activate();
    assert!(director.update(0.016).is_none());
    drop(director);
    drop(im);
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

    let runtime = ScriptHost::new();
    package()
        .ensure_loaded(&runtime, "init")
        .expect("yaobow app script should compile");
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
    let app_handle = runtime.root(app_data);

    let app_data = runtime
        .deref_handle(app_handle)
        .expect("yaobow app root should stay live");
    let title_data = runtime
        .call_method_returning_data(app_data, "make_title_director", Vec::new())
        .expect("title director creation should succeed");
    let handle = host_runtime_handle(&runtime);
    let title_director: ComRc<IDirector> =
        wrap_director(&handle, title_data).expect("wrap_director should succeed");
    drop(title_director);

    let session = shared::openpal4::pal4_debug::create_debug_session();
    let ctx_id = runtime.intern(session.context);
    let ctx_box = runtime
        .foreign_box(
            "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
            ctx_id,
        )
        .expect("IPal4DebugContext foreign box must construct");
    let app_data = runtime
        .deref_handle(app_handle)
        .expect("yaobow app root should stay live");
    runtime
        .call_method_returning_data(app_data, "make_pal4_debug_overlay", vec![ctx_box])
        .expect("PAL4 debug overlay creation should succeed");
}
