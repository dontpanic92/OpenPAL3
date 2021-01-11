#![feature(arbitrary_self_types)]
mod scene;

use nfd::Response;
use radiance::{application::utils::FpsCounter, scene::CoreScene};
use radiance::application::{Application, ApplicationExtension};

struct ApplicationCallbacks {
    path: String,
    fps_counter: FpsCounter,
}

impl ApplicationExtension<ApplicationCallbacks> for ApplicationCallbacks {
    fn on_initialized(&mut self, app: &mut Application<ApplicationCallbacks>) {
        let factory = app.engine_mut().rendering_component_factory();
        app.engine_mut()
            .scene_manager().push_scene(Box::new(CoreScene::new(scene::ScnScene::new(self.path.clone(), factory))));
    }

    fn on_updating(&mut self, app: &mut Application<ApplicationCallbacks>, delta_sec: f32) {
        let fps = self.fps_counter.update_fps(delta_sec);
        let title = format!("Scene Viewer - OpenPAL3 Tools - FPS: {}", fps);
        app.set_title(&title);
    }
}

impl ApplicationCallbacks {
    pub fn new(path: String) -> Self {
        ApplicationCallbacks {
            path,
            fps_counter: FpsCounter::new(),
        }
    }
}

fn main() {
    let mut application = Application::new(ApplicationCallbacks::new("/scene/Q01/y.scn".to_owned()));
    application.initialize();
    application.run();
}
