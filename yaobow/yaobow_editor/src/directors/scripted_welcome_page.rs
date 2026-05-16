//! Scripted welcome page bootstrap.
//!
//! Loads the composed editor script, wires the `EditorHostContext`,
//! calls `init(host)`, and reverse-wraps the returned director box
//! via [`wrap_im_director`] into a `ComRc<IImmediateDirector>`. The
//! Phase 5 [`ImguiImmediateDirectorPump`] is installed on the
//! engine so `CoreRadianceEngine::update` drives `render_im` inside
//! the imgui frame scope. The QI'd `ComRc<IDirector>` is handed
//! back to the caller for `SceneManager::set_director`.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IDirector};
use radiance_scripting::comdef::immediate_director::IUiHost;
use radiance_scripting::services::ui_host::ImguiUiHost;
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::{
    with_services, wrap_im_director, ImguiImmediateDirectorPump, RuntimeAccess, RuntimeHandle,
    ScriptHost,
};
use shared::config::YaobowConfig;

use crate::directors::app_service::AppService;
use crate::directors::config_service::ConfigService;
use crate::editor_bindings::EDITOR_SERVICES_P7;
use crate::script_source::{register_editor_modules, MAIN_P7};
use crate::services::editor_host_context::EditorHostContext;

pub struct ScriptedWelcomePage;

impl ScriptedWelcomePage {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        let config = Rc::new(RefCell::new(YaobowConfig::load()));

        let host = {
            let engine = app.engine();
            let engine = engine.borrow();
            ScriptHost::install(&engine)
        };

        // Register editor-only IDL bindings + each sibling .p7 module.
        // All add_binding registrations survive ScriptHost::reload.
        host.add_binding("yaobow_editor_services", EDITOR_SERVICES_P7);
        register_editor_modules(&host);

        host.load_source(MAIN_P7)
            .expect("editor script must load successfully");

        let textures = build_texture_cache(&app);

        let app_service = AppService::create_with_textures(
            app.clone(),
            config.clone(),
            host.clone(),
            textures.clone(),
        );
        let config_service = ConfigService::create(config.clone());

        let host_ctx = {
            let engine = app.engine();
            let engine = engine.borrow();
            EditorHostContext::create_welcome(
                engine.scene_manager(),
                engine.audio_engine(),
                engine.rendering_component_factory(),
                engine.input_engine(),
                app_service,
                config_service,
                textures.clone(),
                engine.rendering_engine(),
            )
        };

        let host_id = host.intern(host_ctx);
        let host_box = host
            .foreign_box(
                "yaobow_editor.comdef.editor_services.IEditorHostContext",
                host_id,
            )
            .expect("IEditorHostContext foreign box must construct");
        let director_data = host
            .call_returning_data("init", vec![host_box])
            .expect("welcome script init must succeed");

        // Reverse-wrap the script-side `box<radiance.IImmediateDirector>`
        // through the runtime-typed CCW factory. The resulting ComRc
        // QIs back to `ComRc<IDirector>` for SceneManager.
        let runtime_handle = host_runtime_handle(&host);
        let im = wrap_im_director(&runtime_handle, director_data)
            .expect("wrap_im_director must succeed");
        let director: ComRc<IDirector> = im
            .query_interface::<IDirector>()
            .expect("IImmediateDirector must QI to IDirector");

        // Install the engine-side pump (Phase 5 D1/D2/D3) so
        // `CoreRadianceEngine::update` invokes `render_im` inside
        // the imgui frame scope each tick.
        let engine_rc = app.engine();
        let engine = engine_rc.borrow();
        let ui_manager = engine.ui_manager();
        let ui_host: ComRc<IUiHost> = ImguiUiHost::create();
        let pump = Rc::new(ImguiImmediateDirectorPump::new(
            ui_manager,
            textures,
            ui_host,
        ));
        engine.clear_immediate_director_pump();
        engine.set_immediate_director_pump(pump);

        director
    }
}

fn build_texture_cache(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    Rc::new(RefCell::new(ImguiTextureCache::new(factory)))
}

/// Pull a `RuntimeHandle` out of the `ScriptHost`'s services bundle
/// by entering its `RuntimeAccess` scope once.
fn host_runtime_handle(host: &Rc<ScriptHost>) -> RuntimeHandle {
    let mut out = None;
    <ScriptHost as RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = with_services(|s| s.runtime_handle())
            .expect("with_services inside RuntimeAccess scope");
        out = Some(h);
    });
    out.expect("RuntimeAccess::with_ctx ran body")
}
