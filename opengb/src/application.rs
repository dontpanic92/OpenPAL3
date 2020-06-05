pub use radiance::application::{Application, ApplicationExtension};

use crate::{asset_manager::AssetManager, config::OpenGbConfig, director::SceDirector};
use radiance::application::utils::FpsCounter;
use std::path::PathBuf;
use std::rc::Rc;

pub struct OpenGbApplication {
    root_path: PathBuf,
    app_name: String,
    fps_counter: FpsCounter,
    asset_mgr: Option<Rc<AssetManager>>,
}

impl ApplicationExtension<OpenGbApplication> for OpenGbApplication {
    fn on_initialized(&mut self, app: &mut Application<OpenGbApplication>) {
        simple_logger::init().unwrap();
        app.set_title(&self.app_name);
        self.asset_mgr = Some(Rc::new(AssetManager::new(app.engine_mut().rendering_component_factory(), &self.root_path)));

        let sce_director = Box::new(SceDirector::new(
            app.engine_mut().audio_engine_mut(),
            self.asset_mgr.unwrap().load_sce("Q01"),
            1001,
            self.asset_mgr.as_ref().unwrap(),
        ));
        app.engine_mut().set_director(sce_director);

        let scn = self.asset_mgr.unwrap().load_scn("Q01", "yn09a");
        app.engine_mut().load_scene2(scn, 30.);
    }

    fn on_updating(&mut self, app: &mut Application<OpenGbApplication>, delta_sec: f32) {
        let fps = self.fps_counter.update_fps(delta_sec);
    }
}

impl OpenGbApplication {
    pub fn create(config: &OpenGbConfig, app_name: &str) -> Application<OpenGbApplication> {
        Application::new(Self::new(config, app_name))
    }

    fn new(config: &OpenGbConfig, app_name: &str) -> Self {
        let root_path = PathBuf::from(&config.asset_path);

        OpenGbApplication {
            root_path,
            app_name: app_name.to_owned(),
            fps_counter: FpsCounter::new(),
            asset_mgr: None,
        }
    }
}
