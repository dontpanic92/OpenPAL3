use crate::ComObject_OpenPal3ApplicationLoaderComponent;

use super::debug_layer::OpenPal3DebugLayer;
use super::main_menu_director;

use crosscom::ComRc;
use opengb::video::register_opengb_video_decoders;
use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::Application;
use radiance::comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl};
use std::rc::Rc;
use std::{cell::RefCell, path::PathBuf};

pub struct OpenPal3ApplicationLoader {
    app: ComRc<IApplication>,
    root_path: PathBuf,
    app_name: String,
}

ComObject_OpenPal3ApplicationLoaderComponent!(super::OpenPal3ApplicationLoader);

impl IComponentImpl for OpenPal3ApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title(&self.app_name);

        register_opengb_video_decoders();

        let input_engine = self.app.engine().borrow().input_engine();
        let audio_engine = self.app.engine().borrow().audio_engine();
        let asset_mgr = Rc::new(AssetManager::new(
            self.app.engine().borrow().rendering_component_factory(),
            &self.root_path,
            None,
        ));

        let debug_layer = OpenPal3DebugLayer::new(input_engine.clone(), audio_engine.clone());
        self.app
            .engine()
            .borrow_mut()
            .set_debug_layer(Box::new(debug_layer));

        let director = main_menu_director::MainMenuDirector::new(
            asset_mgr.clone(),
            audio_engine,
            input_engine,
        );
        self.app
            .engine()
            .borrow_mut()
            .scene_manager()
            .set_director(Rc::new(RefCell::new(director)));
    }

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenPal3ApplicationLoader {
    pub fn create_application(config: &OpenGbConfig, app_name: &str) -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone(), config, app_name)),
        );

        app
    }

    fn new(app: ComRc<IApplication>, config: &OpenGbConfig, app_name: &str) -> Self {
        let root_path = PathBuf::from(&config.asset_path);

        Self {
            app,
            root_path,
            app_name: app_name.to_owned(),
        }
    }
}
