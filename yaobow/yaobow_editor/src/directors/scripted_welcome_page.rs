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
    ScriptHost, bootstrap_script_root, install_imgui_pump_with_cache, wrap_director,
};
use shared::config::YaobowConfig;

use crate::directors::app_service::AppService;
use crate::script_source::EDITOR_PACKAGE;
use crate::services::editor_host_context::EditorHostContext;
use shared::config_service::ConfigService;

/// `type_tag` for the editor-specific `IEditorHostContext`. Hard-coded
/// until per-interface tag codegen lands (step 5 of the script-source
/// refactor). Mirrors the generated `<crate>.comdef.<module>.<Iface>`
/// shape that `crosscom_protosept::interface_type_tag` produces.
const EDITOR_HOST_CONTEXT_TYPE_TAG: &str =
    "yaobow_editor.comdef.editor_services.IEditorHostContext";

pub struct ScriptedWelcomePage;

impl ScriptedWelcomePage {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        let config = Rc::new(RefCell::new(YaobowConfig::load()));

        let host = {
            let engine = app.engine();
            let engine = engine.borrow();
            ScriptHost::install(&engine)
        };

        let textures = build_texture_cache(&app);

        let app_service = AppService::create_with_textures(
            app.clone(),
            config.clone(),
            host.clone(),
            textures.clone(),
        );
        let imgui_ctx = app.engine().borrow().ui_manager().imgui_context();
        let config_service = ConfigService::create_with_imgui(config.clone(), Some(imgui_ctx));

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

        // Register every IDL binding + sibling .p7 module declared in
        // EDITOR_PACKAGE, compile main.p7 if not already loaded,
        // intern the host context, then call `init(host)` on the root
        // module. The `bootstrap_script_root` helper unifies the
        // ensure_loaded → intern → foreign_box → call pipeline so we
        // don't repeat it in each per-binary bootstrap.
        let director_data = bootstrap_script_root(
            &host,
            &EDITOR_PACKAGE,
            host_ctx,
            EDITOR_HOST_CONTEXT_TYPE_TAG,
            "init",
        )
        .expect("welcome script init must succeed");

        // Reverse-wrap the script-side `box<radiance.IImmediateDirector>`
        // through the runtime-typed CCW factory. The fat CCW gives us
        // both `IDirector` (for `SceneManager::set_director`) and
        // `IImmediateDirector` (for the imgui pump) from a single
        // `wrap_director` call, because the script struct's
        // `conforming_to` list backs every advertised interface with
        // its own slot.
        let runtime_handle = host.runtime_handle();
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
