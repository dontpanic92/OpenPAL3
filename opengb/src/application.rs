use radiance::application::{Application, ApplicationExtension};
use radiance::application::utils::FpsCounter;
use crate::{resource_manager::ResourceManager, scene::ScnScene};
use std::path::{Path, PathBuf};

pub struct OpenGbApplication {
    root_path: PathBuf,
    app_name: String,
    fps_counter: FpsCounter,
    res_man: ResourceManager,
}

impl ApplicationExtension<OpenGbApplication> for OpenGbApplication {
    fn on_initialized(
        &mut self,
        app: &mut Application<OpenGbApplication>,
    ) {
        app.set_title(&self.app_name);

        let scn = self.res_man.load_scn("Q01", "yn09a");
        app.engine_mut()
            .load_scene2(scn);
    }

    fn on_updated(
        &mut self,
        app: &mut Application<OpenGbApplication>,
        delta_sec: f32,
    ) {
        let fps = self.fps_counter.update_fps(delta_sec);
    }
}

impl OpenGbApplication {
    pub fn create<P: AsRef<Path>>(root_path: P, app_name: &str) -> Application<OpenGbApplication> {
        Application::new(Self::new(root_path, app_name))
    }

    fn new<P: AsRef<Path>>(root_path: P, app_name: &str) -> Self {
        OpenGbApplication {
            root_path: root_path.as_ref().to_owned(),
            app_name: app_name.to_owned(),
            fps_counter: FpsCounter::new(),
            res_man: ResourceManager::new(root_path),
        }
    }
}