pub mod yaobow_app_context;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{
        IApplication, IApplicationExt, IApplicationLoaderComponent, IComponent, IComponentImpl,
    },
    scene::CoreScene,
};
use radiance_scripting::install_imgui_pump;
use shared::{config::YaobowConfig, GameType};

use crate::script_source::YaobowScriptProject;
use crate::{
    openpal3::OpenPal3ApplicationLoader, openpal4::application::OpenPal4ApplicationLoader,
    openswd5::application::OpenSwd5ApplicationLoader,
};

pub struct YaobowApplicationLoader {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    selected_game: RefCell<Option<Rc<RefCell<Option<GameType>>>>>,
}

ComObject_YaobowApplicationLoader!(super::YaobowApplicationLoader);

impl IComponentImpl for YaobowApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title("妖弓 - Project Yaobow");

        let project = YaobowScriptProject::install(&self.app, self.config.clone());
        self.selected_game.replace(Some(project.selected_game()));

        let director = project
            .make_title_director_as_director()
            .expect("initial script director must be created");

        let _ = install_imgui_pump(&self.app);

        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        scene_manager.set_director(director);
        scene_manager.push_scene(CoreScene::create());
    }

    fn on_updating(&self, _delta_sec: f32) {
        let slot = self.selected_game.borrow();
        let Some(slot) = slot.as_ref() else {
            return;
        };
        if slot.borrow().is_none() {
            return;
        }

        let game = slot.borrow().unwrap();
        let asset_path = self.config.borrow().asset_path_for(game).to_string();
        let loader = create_loader(game, self.app.clone(), asset_path)
            .query_interface::<IComponent>()
            .unwrap();

        loader.on_loading();

        slot.replace(None);
    }

    fn on_unloading(&self) {}
}

impl YaobowApplicationLoader {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            config: Rc::new(RefCell::new(YaobowConfig::load())),
            selected_game: RefCell::new(None),
        }
    }
}

fn create_loader(
    game: GameType,
    app: ComRc<IApplication>,
    asset_path: String,
) -> ComRc<IApplicationLoaderComponent> {
    match game {
        GameType::PAL3 => OpenPal3ApplicationLoader::create(app, &asset_path),
        GameType::PAL4 => OpenPal4ApplicationLoader::create(app, asset_path),
        GameType::SWDHC => OpenSwd5ApplicationLoader::create(app, asset_path),
        _ => unimplemented!(),
    }
}

pub fn create_application() -> ComRc<IApplication> {
    let app = ComRc::<IApplication>::from_object(Application::new());
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(YaobowApplicationLoader::new(app.clone())),
    );

    app
}

pub fn run_title_selection() {
    let app = create_application();
    app.initialize();
    shared::theme_runtime::apply_runtime_theme(&app);
    app.run();
}
