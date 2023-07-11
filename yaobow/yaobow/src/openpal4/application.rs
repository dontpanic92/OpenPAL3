use std::{cell::RefCell, path::PathBuf};

use crosscom::{ComObject, ComRc};
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
};
use shared::{
    fs::init_virtual_fs,
    openpal4::{
        app_context::Pal4AppContext, asset_loader::AssetLoader, director::OpenPAL4Director,
        scripting::create_script_vm,
    },
    scripting::angelscript::ScriptVm,
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
        self.app.set_title(&self.app_name);

        let input_engine = self.app.engine().borrow().input_engine();
        let audio_engine = self.app.engine().borrow().audio_engine();
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        let ui = self.app.engine().borrow().ui_manager();

        let vfs = init_virtual_fs(self.root_path.to_str().unwrap(), None);
        let loader = AssetLoader::new(
            self.app.engine().borrow().rendering_component_factory(),
            vfs,
        );

        let director = OpenPAL4Director::new(loader, scene_manager.clone(), ui);
        scene_manager.set_director(ComRc::from_object(director));
    }

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

    fn new(app: ComRc<IApplication>, app_name: &str) -> Self {
        Self {
            app,
            root_path: PathBuf::from("F:\\PAL4"),
            app_name: app_name.to_owned(),
        }
    }
}
