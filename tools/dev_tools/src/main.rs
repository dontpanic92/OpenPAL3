#![feature(arbitrary_self_types)]
#![feature(drain_filter)]
mod components;
mod content;
mod director;
mod scene;

use std::{cell::RefCell, rc::Rc};

use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::{Application, ApplicationExtension};
use radiance::scene::CoreScene;
use scene::DevToolsScene;

struct ApplicationCallbacks {
    config: OpenGbConfig,
}

impl ApplicationExtension<ApplicationCallbacks> for ApplicationCallbacks {
    fn on_initialized(&mut self, app: &mut Application<ApplicationCallbacks>) {
        let factory = app.engine_mut().rendering_component_factory();

        let asset_mgr = AssetManager::new(factory, &self.config.asset_path);
        let input_engine = app.engine_mut().input_engine();
        let audio_engine = app.engine_mut().audio_engine();

        app.engine_mut()
            .scene_manager()
            .push_scene(Box::new(CoreScene::new(DevToolsScene::new())));
        app.engine_mut()
            .scene_manager()
            .set_director(Rc::new(RefCell::new(director::DevToolsDirector::new(
                input_engine,
                audio_engine,
                Rc::new(asset_mgr),
            ))))
    }

    fn on_updating(&mut self, app: &mut Application<ApplicationCallbacks>, delta_sec: f32) {}
}

impl ApplicationCallbacks {
    pub fn new(config: OpenGbConfig) -> Self {
        ApplicationCallbacks { config }
    }
}

fn main() {
    let config = OpenGbConfig::load("openpal3", "OPENPAL3");
    let mut application = Application::new(ApplicationCallbacks::new(config));
    application.initialize();
    application.run();
}
