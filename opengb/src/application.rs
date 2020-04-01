use crate::{director::SceDirector, resource_manager::ResourceManager};
use radiance::application::utils::FpsCounter;
use radiance::application::{Application, ApplicationExtension};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct OpenGbApplication {
    root_path: PathBuf,
    app_name: String,
    fps_counter: FpsCounter,
    res_man: Rc<ResourceManager>,
}

impl ApplicationExtension<OpenGbApplication> for OpenGbApplication {
    fn on_initialized(&mut self, app: &mut Application<OpenGbApplication>) {
        app.set_title(&self.app_name);

        let sce_director = Box::new(SceDirector::new(&self.res_man));
        app.engine_mut().set_director(sce_director);

        let scn = self.res_man.load_scn("Q01", "yn09a");
        app.engine_mut().load_scene2(scn, 30.);
    }

    fn on_updating(&mut self, app: &mut Application<OpenGbApplication>, delta_sec: f32) {
        let fps = self.fps_counter.update_fps(delta_sec);
    }
}

impl OpenGbApplication {
    pub fn create<P: AsRef<Path>>(root_path: P, app_name: &str) -> Application<OpenGbApplication> {
        Application::new(Self::new(root_path, app_name))
    }

    fn new<P: AsRef<Path>>(root_path: P, app_name: &str) -> Self {
        let root_path = root_path.as_ref().to_owned();
        let res_man = Rc::new(ResourceManager::new(&root_path));

        OpenGbApplication {
            root_path,
            app_name: app_name.to_owned(),
            fps_counter: FpsCounter::new(),
            res_man,
        }
    }
}
