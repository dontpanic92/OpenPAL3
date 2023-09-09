use std::{path::PathBuf, rc::Rc};

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
};
use shared::{
    config::YaobowConfig,
    fs::init_virtual_fs,
    openpal5::{asset_loader::AssetLoader, director::OpenPAL5Director, scene::Pal5Scene},
};

use crate::ComObject_OpenPal5ApplicationLoaderComponent;

pub struct OpenPal5ApplicationLoader {
    app: ComRc<IApplication>,
    root_path: PathBuf,
}

ComObject_OpenPal5ApplicationLoaderComponent!(super::OpenPal5ApplicationLoader);

impl IComponentImpl for OpenPal5ApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title(&format!("OpenPAL5 - Project Yaobow"));

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
        );

        let scene = Pal5Scene::load(&loader, "kuangfengzhai").unwrap();
        scene_manager.push_scene(scene.scene);

        let director = OpenPAL5Director::new(input_engine.clone());
        scene_manager.set_director(ComRc::from_object(director));
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenPal5ApplicationLoader {
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
            root_path: PathBuf::from("F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5"),
        }
    }
}
