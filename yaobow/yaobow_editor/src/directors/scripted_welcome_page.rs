//! Scripted welcome page bootstrap.
//!
//! Loads the composed editor script, wires the `EditorHostContext`,
//! calls `init(host)`, and reverse-wraps the returned director box
//! via [`wrap_director`] into a `ComRc<IDirector>`. The fat CCW
//! backs both `IDirector` and `IImmediateDirector` from a single
//! wrap call, so the engine's [`ImguiImmediateDirectorPump`] (set up
//! via [`install_imgui_pump_with_cache`]) can QI the same director
//! to drive its `render_im` step inside the imgui frame scope.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::{
    install_imgui_pump_with_cache, with_services, wrap_director, RuntimeAccess, RuntimeHandle,
    ScriptHost,
};
use shared::config::YaobowConfig;

use crate::directors::app_service::AppService;
use crate::editor_bindings::EDITOR_SERVICES_P7;
use crate::script_source::{register_editor_modules, MAIN_P7};
use crate::services::editor_host_context::EditorHostContext;
use shared::config_service::ConfigService;

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
        let imgui_ctx = app.engine().borrow().ui_manager().imgui_context();
        let config_service =
            ConfigService::create_with_imgui(config.clone(), Some(imgui_ctx));

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
        // through the runtime-typed CCW factory. The fat CCW gives us
        // both `IDirector` (for `SceneManager::set_director`) and
        // `IImmediateDirector` (for the imgui pump) from a single
        // `wrap_director` call, because the script struct's
        // `conforming_to` list backs every advertised interface with
        // its own slot.
        let runtime_handle = host_runtime_handle(&host);
        let director: ComRc<IDirector> =
            wrap_director(&runtime_handle, director_data).expect("wrap_director must succeed");

        // Install the engine-side pump so `CoreRadianceEngine::update`
        // invokes `render_im` inside the imgui frame scope each tick.
        // Sharing the same `textures` cache with subsequent director
        // swaps (`AppService::open_game`) keeps previewer com_ids
        // resolvable across pages.
        install_imgui_pump_with_cache(&app, textures);

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
