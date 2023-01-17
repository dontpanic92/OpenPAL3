use crate::debug_layer::OpenPal3DebugLayer;
use crate::main_menu_director;

use opengb::video::register_opengb_video_decoders;
use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::{Application, ApplicationExtension};
use std::rc::Rc;
use std::{cell::RefCell, path::PathBuf};

pub struct OpenPal3Application {
    root_path: PathBuf,
    app_name: String,
    asset_mgr: Option<Rc<AssetManager>>,
}

impl ApplicationExtension<OpenPal3Application> for OpenPal3Application {
    fn on_initialized(&mut self, app: &mut Application<OpenPal3Application>) {
        let logger = simple_logger::SimpleLogger::new();
        // workaround panic on Linux for 'Could not determine the UTC offset on this system'
        // see: https://github.com/borntyping/rust-simple_logger/issues/47
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
        let logger = logger.with_utc_timestamps();
        logger.init().unwrap();

        app.set_title(&self.app_name);

        register_opengb_video_decoders();

        let input_engine = app.engine_mut().input_engine();
        let audio_engine = app.engine_mut().audio_engine();
        self.asset_mgr = Some(Rc::new(AssetManager::new(
            app.engine_mut().rendering_component_factory(),
            &self.root_path,
            None,
        )));

        let debug_layer = OpenPal3DebugLayer::new(input_engine.clone(), audio_engine.clone());
        app.engine_mut().set_debug_layer(Box::new(debug_layer));

        let director = main_menu_director::MainMenuDirector::new(
            self.asset_mgr.as_ref().unwrap().clone(),
            audio_engine,
            input_engine,
        );
        app.engine_mut()
            .scene_manager()
            .set_director(Rc::new(RefCell::new(director)));
    }

    fn on_updating(&mut self, _app: &mut Application<OpenPal3Application>, _delta_sec: f32) {}
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
            asset_mgr: None,
        }
    }
}
