use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationExt, IApplicationLoaderComponent, IComponentImpl},
};
use radiance_scripting::install_imgui_pump;
use shared::config::YaobowConfig;
use shared::openpal4::{asset_loader::AssetLoader, director::OpenPAL4Director};

use crate::script_source::YaobowScriptProject;

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

        // Create the PAL4 debug-overlay session before the director is
        // constructed; the resulting bundle is handed to the director
        // so its `render_im` can dispatch into the script-side overlay
        // each frame. `YaobowScriptProject::install` is idempotent —
        // if the title bootstrap already installed the project, this
        // call just returns the cached `Rc<YaobowScriptProject>`.
        let config = Rc::new(RefCell::new(YaobowConfig::load()));
        let project = YaobowScriptProject::install(&self.app, config);
        let debug = project.make_pal4_debug_bundle();
        let actor_controller_factory = project.actor_controller_factory();

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager.clone(),
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );
        director.set_debug_bundle(debug);
        director.set_actor_controller_factory(actor_controller_factory);
        let director_com: ComRc<radiance::comdef::IDirector> = ComRc::from_object(director);
        scene_manager.set_director(director_com);

        // Install the engine-side imgui pump so
        // `OpenPAL4Director::render_im` fires inside the imgui frame
        // scope each tick. The texture cache is wired even though the
        // v1 debug overlay only emits text — keeps parity with the
        // editor's pump and future-proofs `ui.image(...)` from script.
        let _ = install_imgui_pump(&self.app);
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenPal4ApplicationLoader {
    pub fn create_application(asset_path: String, app_name: &str) -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone(), asset_path, app_name)),
        );

        app
    }

    pub fn create(
        app: ComRc<IApplication>,
        asset_path: String,
    ) -> ComRc<IApplicationLoaderComponent> {
        ComRc::from_object(Self::new(app.clone(), asset_path, "OpenPAL4"))
    }

    fn new(app: ComRc<IApplication>, asset_path: String, app_name: &str) -> Self {
        let root_path = if cfg!(vita) {
            PathBuf::from("ux0:games/PAL4")
        } else if !asset_path.is_empty() {
            PathBuf::from(asset_path)
        } else {
            PathBuf::from("F:\\PAL4_test")
        };
        Self {
            app,
            root_path,
            app_name: app_name.to_owned(),
        }
    }
}
