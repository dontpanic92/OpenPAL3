mod scene;

use nfd::Response;
use radiance::application::utils::FpsCounter;
use radiance::application::{Application, ApplicationExtension};

struct ApplicationCallbacks {
    path: String,
    fps_counter: FpsCounter,
}

impl ApplicationExtension<ApplicationCallbacks> for ApplicationCallbacks {
    fn on_initialized(&mut self, app: &mut Application<ApplicationCallbacks>) {
        app.engine_mut()
            .load_scene2(scene::ScnScene::new(self.path.clone()), 90.);
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
    let result = nfd::open_file_dialog(Some("scn"), None).unwrap_or_else(|e| {
        panic!(e);
    });

    let path = match result {
        Response::Okay(file_path) => file_path,
        Response::OkayMultiple(files) => String::from(&files[0]),
        Response::Cancel => std::process::exit(0),
    };

    let mut application = Application::new(ApplicationCallbacks::new(path));
    application.initialize();
    application.run();
}
