//! Scripted welcome page bootstrap.
//!
//! Single entry point that wires the editor's `ScriptHost` (engine service),
//! `AppService`, and `ConfigService` together, loads `welcome.p7`, calls its
//! `init(host)` entry point, and wraps the returned script director in a
//! `ScriptedDirector` proxy ready to be pushed onto the engine.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IDirector};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::{ScriptHost, ScriptedDirector};
use shared::config::YaobowConfig;

use crate::directors::app_service::build_host_context;

const WELCOME_SCRIPT: &str = include_str!("../../scripts/welcome.p7");

pub struct ScriptedWelcomePage;

impl ScriptedWelcomePage {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        let config = Rc::new(RefCell::new(YaobowConfig::load()));

        let host = {
            let engine = app.engine();
            let engine = engine.borrow();
            ScriptHost::install(&engine)
        };
        host.load_source(WELCOME_SCRIPT)
            .expect("welcome.p7 must load successfully");

        let host_ctx = build_host_context(&app, config, host.clone());
        let host_id = host.intern(host_ctx);
        let host_box = host
            .foreign_box("radiance_scripting.comdef.services.IHostContext", host_id)
            .expect("IHostContext foreign box must construct");
        let director = host
            .call_returning_data("init", vec![host_box])
            .expect("welcome.p7 must initialize successfully");
        let handle = host.root(director);

        let ui_manager = app.engine().borrow().ui_manager();
        let textures = build_texture_cache(&app);
        ScriptedDirector::with_ui(host, handle, ui_manager, textures)
    }
}

fn build_texture_cache(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    Rc::new(RefCell::new(ImguiTextureCache::new(factory)))
}
