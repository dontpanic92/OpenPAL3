use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponentImpl},
};
use radiance_scripting::comdef::immediate_director::IUiHost;
use radiance_scripting::services::ui_host::ImguiUiHost;
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::ImguiImmediateDirectorPump;
use shared::openpal4::{asset_loader::AssetLoader, director::OpenPAL4Director};

use super::debug_bootstrap;

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

        // Bootstrap the protosept-authored debug overlay before the
        // director is constructed; the resulting bundle is handed to
        // the director so its `render_im` can dispatch into the
        // script-side overlay each frame.
        let debug = {
            let engine_rc = self.app.engine();
            let engine = engine_rc.borrow();
            debug_bootstrap::install(&engine)
        };

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager.clone(),
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );
        director.set_debug_bundle(debug.bundle);
        let director_com: ComRc<radiance::comdef::IDirector> = ComRc::from_object(director);
        scene_manager.set_director(director_com);

        // Install the imgui immediate-mode pump on the engine so
        // `OpenPAL4Director::render_im` fires inside the imgui frame
        // scope each tick. The texture cache is wired even though the
        // v1 debug overlay only emits text — keeps parity with the
        // editor's pump and future-proofs `ui.image(...)` from script.
        let textures = Rc::new(RefCell::new(ImguiTextureCache::new(component_factory)));
        let ui_host: ComRc<IUiHost> = ImguiUiHost::create();
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let pump = Rc::new(ImguiImmediateDirectorPump::new(
            engine.ui_manager(),
            textures,
            ui_host,
        ));
        engine.clear_immediate_director_pump();
        engine.set_immediate_director_pump(pump);

        // Hold onto the ScriptHost Rc; the engine also keeps it via
        // `ScriptHost::install`, so dropping ours here is fine, but we
        // forget it explicitly to make the lifetime intent obvious.
        let _ = debug.host;
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
