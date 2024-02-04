use std::{path::PathBuf, rc::Rc};

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
    scene::CoreScene,
};
use shared::{
    config::YaobowConfig,
    fs::init_virtual_fs,
    openswd5::{asset_loader::AssetLoader, director::OpenSWD5Director},
};

use crate::ComObject_OpenSwd5ApplicationLoaderComponent;

pub struct OpenSwd5ApplicationLoader {
    app: ComRc<IApplication>,
    root_path: PathBuf,
}

ComObject_OpenSwd5ApplicationLoaderComponent!(super::OpenSwd5ApplicationLoader);

impl IComponentImpl for OpenSwd5ApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title(&format!("OpenSWD5 - Project Yaobow"));

        let component_factory = self.app.engine().borrow().rendering_component_factory();
        let input_engine = self.app.engine().borrow().input_engine();
        let task_manager = self.app.engine().borrow().task_manager();
        let audio_engine = self.app.engine().borrow().audio_engine();
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        let ui = self.app.engine().borrow().ui_manager();

        let vfs = init_virtual_fs(self.root_path.to_str().unwrap(), None);
        let loader = AssetLoader::new(
            self.app.engine().borrow().rendering_component_factory(),
            Rc::new(vfs),
            shared::GameType::SWDHC,
        );

        let scene = CoreScene::create();
        scene_manager.push_scene(scene);

        let director = OpenSWD5Director::new(
            loader,
            input_engine.clone(),
            audio_engine,
            component_factory.clone(),
            ui.clone(),
        );
        scene_manager.set_director(ComRc::from_object(director));
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenSwd5ApplicationLoader {
    pub fn create_application() -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone())),
        );

        app
    }

    pub fn create(
        app: ComRc<IApplication>,
        _config: YaobowConfig,
    ) -> ComRc<IApplicationLoaderComponent> {
        ComRc::from_object(Self::new(app.clone()))
    }

    fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            root_path: PathBuf::from("F:\\SteamLibrary\\steamapps\\common\\SWDHC"),
        }
    }
}
