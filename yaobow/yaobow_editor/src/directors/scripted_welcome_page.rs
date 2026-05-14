//! Scripted welcome page bootstrap.
//!
//! Loads the composed editor script (welcome + main editor + content tabs +
//! resource tree + previewers), wires the `EditorHostContext`, calls
//! `init(host)`, and wraps the returned director in a `ScriptedDirector`
//! ready to be pushed onto the engine.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IDirector};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::{ScriptHost, ScriptedDirector};
use shared::config::YaobowConfig;

use crate::directors::app_service::AppService;
use crate::directors::config_service::ConfigService;
use crate::editor_bindings::EDITOR_SERVICES_P7;
use crate::script_source::{MAIN_P7, register_editor_modules};
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
            )
        };

        let host_id = host.intern(host_ctx);
        let host_box = host
            .foreign_box(
                "yaobow_editor.comdef.editor_services.IEditorHostContext",
                host_id,
            )
            .expect("IEditorHostContext foreign box must construct");
        let director = host
            .call_returning_data("init", vec![host_box])
            .expect("welcome script init must succeed");
        let handle = host.root(director);

        let ui_manager = app.engine().borrow().ui_manager();
        ScriptedDirector::with_ui(host, handle, ui_manager, textures)
    }
}

fn build_texture_cache(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    Rc::new(RefCell::new(ImguiTextureCache::new(factory)))
}
