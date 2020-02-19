mod scene;

use radiance::application;
use radiance::scene::CoreScene;
use nfd::Response;

struct ApplicationCallbacks {
    pub path: String,
}

impl application::ApplicationCallbacks for ApplicationCallbacks {
    fn on_initialized<T: application::ApplicationCallbacks>(&mut self, app: &mut application::Application<T>) {
        app.engine_mut().load_scene(CoreScene::new(scene::ModelViewerScene { path: self.path.clone() }));
    }

    fn on_updated<T: application::ApplicationCallbacks>(&mut self, app: &mut application::Application<T>, delta_sec: f32) {
        let title = format!("MV3 Animation Viewer - OpenPAL3 Tools - FPS: {}", 1./delta_sec);
        app.set_title(&title);
    }
}

fn main() {
    let result = nfd::open_file_dialog(None, None).unwrap_or_else(|e| {
        panic!(e);
    });
  
    let path = match result {
        Response::Okay(file_path) => file_path,
        Response::OkayMultiple(files) => String::from(&files[0]),
        Response::Cancel => std::process::exit(0),
    };

    let mut application = application::Application::new(ApplicationCallbacks { path });
    application.initialize();
    application.run();
}
