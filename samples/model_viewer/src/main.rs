mod scene;

use radiance::application;
use radiance::scene::CoreScene;
use std::rc::Rc;
use std::cell::RefCell;

struct ApplicationCallbacks {}

impl application::ApplicationCallbacks for ApplicationCallbacks {
    fn on_initialized<T: application::ApplicationCallbacks>(&mut self, app: &mut application::Application<T>) {
        app.engine_mut().load_scene(CoreScene::new(RefCell::new(scene::ModelViewerScene{})));
    }

    fn on_updated<T: application::ApplicationCallbacks>(&mut self, app: &mut application::Application<T>, delta_sec: f32) {
        let title = format!("Radiance - FPS: {}", 1./delta_sec);
        app.set_title(&title);
    }
}

fn main() {
    let mut application = application::Application::new(RefCell::new(ApplicationCallbacks {}));
    application.initialize();
    application.run();
}
