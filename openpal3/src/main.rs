use log::debug;
use opengb::directors::SceneManagerExtensions;
use opengb::{
    asset_manager::AssetManager,
    config::OpenGbConfig,
    directors::{ExplorationDirector, SceDirector},
};
use radiance::{
    application::utils::FpsCounter,
    math::Vec3,
    scene::{CoreScene, Scene},
};
use radiance::{
    application::{Application, ApplicationExtension},
    input::Key,
};
use std::rc::Rc;
use std::{cell::RefCell, path::PathBuf};

fn main() {
    let config = OpenGbConfig::load("openpal3", "OPENPAL3");
    let mut app = OpenPal3Application::create(&config, "OpenPAL3");
    app.initialize();
    app.run();
}

pub struct OpenPal3Application {
    root_path: PathBuf,
    app_name: String,
    fps_counter: FpsCounter,
    asset_mgr: Option<Rc<AssetManager>>,
}

impl ApplicationExtension<OpenPal3Application> for OpenPal3Application {
    fn on_initialized(&mut self, app: &mut Application<OpenPal3Application>) {
        simple_logger::init().unwrap();
        app.set_title(&self.app_name);

        let input_engine = app.engine_mut().input_engine();
        self.asset_mgr = Some(Rc::new(AssetManager::new(
            app.engine_mut().rendering_component_factory(),
            &self.root_path,
        )));

        let mut scene = Box::new(CoreScene::new(
            self.asset_mgr.as_ref().unwrap().load_scn("Q01", "yn09a"),
        ));
        scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(300., 300., 300.))
            .look_at(&Vec3::new(0., 0., 0.));
        app.engine_mut().scene_manager().push_scene(scene);

        let sce_director = SceDirector::new(
            app.engine_mut().audio_engine(),
            input_engine.clone(),
            self.asset_mgr.as_ref().unwrap().load_sce("Q01"),
            1001,
            self.asset_mgr.as_ref().unwrap().clone(),
        );
        app.engine_mut()
            .scene_manager()
            .scene_mut_or_fail()
            .get_role_entity_mut("101")
            .set_active(true);

        // let exp_director = ExplorationDirector::new(sce_director, input_engine.clone());
        app.engine_mut().scene_manager().set_director(sce_director);
    }

    fn on_updating(&mut self, app: &mut Application<OpenPal3Application>, delta_sec: f32) {
        let fps = self.fps_counter.update_fps(delta_sec);
    }
}

impl OpenPal3Application {
    pub fn create(config: &OpenGbConfig, app_name: &str) -> Application<OpenPal3Application> {
        Application::new(Self::new(config, app_name))
    }

    fn new(config: &OpenGbConfig, app_name: &str) -> Self {
        let root_path = PathBuf::from(&config.asset_path);

        OpenPal3Application {
            root_path,
            app_name: app_name.to_owned(),
            fps_counter: FpsCounter::new(),
            asset_mgr: None,
        }
    }
}
