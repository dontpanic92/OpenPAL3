use std::path::PathBuf;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
};
use shared::{
    config::YaobowConfig,
    openpal4::{asset_loader::AssetLoader, director::OpenPAL4Director},
};

use crate::ComObject_OpenPal4ApplicationLoaderComponent;

pub struct OpenPal4ApplicationLoader {
    app: ComRc<IApplication>,
    root_path: PathBuf,
    app_name: String,
}

ComObject_OpenPal4ApplicationLoaderComponent!(super::OpenPal4ApplicationLoader);

impl IComponentImpl for OpenPal4ApplicationLoader {
    fn on_loading(&self) {
        self.app
            .set_title(&format!("{} - Project Yaobow", &self.app_name));

        let component_factory = self.app.engine().borrow().rendering_component_factory();
        let input_engine = self.app.engine().borrow().input_engine();
        let task_manager = self.app.engine().borrow().task_manager();
        let audio_engine = self.app.engine().borrow().audio_engine();
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        let ui = self.app.engine().borrow().ui_manager();

        let vfs = init_virtual_fs(self.root_path.to_str().unwrap(), None);
        let loader = AssetLoader::new(
            self.app.engine().borrow().rendering_component_factory(),
            input_engine.clone(),
            vfs,
        );

        let director = OpenPAL4Director::new(
            component_factory,
            loader,
            scene_manager.clone(),
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );
        scene_manager.set_director(ComRc::from_object(director));
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenPal4ApplicationLoader {
    pub fn create_application(app_name: &str) -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone(), app_name)),
        );

        app
    }

    pub fn create(
        app: ComRc<IApplication>,
        _config: YaobowConfig,
    ) -> ComRc<IApplicationLoaderComponent> {
        ComRc::from_object(Self::new(app.clone(), "OpenPAL4"))
    }

    fn new(app: ComRc<IApplication>, app_name: &str) -> Self {
        Self {
            app,
            root_path: PathBuf::from("F:\\PAL4_test"), // PathBuf::from("F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 4")
            app_name: app_name.to_owned(),
        }
    }
}
