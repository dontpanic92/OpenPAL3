use std::path::PathBuf;

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
};
use shared::{
    fs::init_virtual_fs,
    openpal4::{asset_loader::AssetLoader, scripting::create_script_vm},
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

        let vfs = init_virtual_fs(self.root_path.to_str().unwrap(), None);
        let loader = AssetLoader::new(vfs);
        let mut vm = create_script_vm(loader);
        vm.execute();
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
