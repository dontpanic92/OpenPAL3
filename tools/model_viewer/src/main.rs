mod mv3entity;
mod polentity;
mod cvdentity;
mod scene;

use nfd::Response;
use radiance::application;
use radiance::application::utils::FpsCounter;
use radiance::scene::CoreScene;

struct ApplicationCallbacks {
    path: String,
    fps_counter: FpsCounter,
}

impl application::ApplicationCallbacks for ApplicationCallbacks {
    fn on_initialized<T: application::ApplicationCallbacks>(
        &mut self,
        app: &mut application::Application<T>,
    ) {
        app.engine_mut()
            .load_scene(CoreScene::new(scene::ModelViewerScene {
                path: self.path.clone(),
            }));
    }

    fn on_updated<T: application::ApplicationCallbacks>(
        &mut self,
        app: &mut application::Application<T>,
        delta_sec: f32,
    ) {
        let fps = self.fps_counter.update_fps(delta_sec);
        let title = format!("Model Viewer - OpenPAL3 Tools - FPS: {}", fps);
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
    let result = nfd::open_file_dialog(Some("mv3,pol,cvd"), None).unwrap_or_else(|e| {
        panic!(e);
    });

    let path = match result {
        Response::Okay(file_path) => file_path,
        Response::OkayMultiple(files) => String::from(&files[0]),
        Response::Cancel => std::process::exit(0),
    };

    let mut application = application::Application::new(ApplicationCallbacks::new(path));
    application.initialize();
    application.run();
}
