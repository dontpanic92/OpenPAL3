mod director;

use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponent, IComponentImpl},
    scene::CoreScene,
};
use shared::{config::YaobowConfig, GameType};

use crate::{
    openpal3::OpenPal3ApplicationLoader, openpal4::application::OpenPal4ApplicationLoader,
    ComObject_YaobowApplicationLoader,
};

use self::director::TitleSelectionDirector;

pub struct YaobowApplicationLoader {
    app: ComRc<IApplication>,
    config: anyhow::Result<YaobowConfig>,
    selected_game: Rc<RefCell<Option<GameType>>>,
}

ComObject_YaobowApplicationLoader!(super::YaobowApplicationLoader);

impl IComponentImpl for YaobowApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title("妖弓 - Project Yaobow");
        let audio = self.app.engine().borrow().audio_engine();
        let dpi_scale = self.app.dpi_scale();
        let factory = self.app.engine().borrow().rendering_component_factory();
        let input = self.app.engine().borrow().input_engine();
        let ui = self.app.engine().borrow().ui_manager();

        let director = TitleSelectionDirector::new(
            factory,
            audio,
            input,
            ui,
            self.selected_game.clone(),
            dpi_scale,
        );
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        scene_manager.set_director(ComRc::from_object(director));
        scene_manager.push_scene(CoreScene::create());
    }

    fn on_updating(&self, delta_sec: f32) {
        if self.selected_game.borrow().is_none() {
            return;
        }

        let game = self.selected_game.borrow().unwrap();
        let loader = create_loader(
            game,
            self.app.clone(),
            self.config.as_ref().cloned().unwrap(),
        )
        .query_interface::<IComponent>()
        .unwrap();

        loader.on_loading();

        self.selected_game.replace(None);
    }

    fn on_unloading(&self) {}
}

impl YaobowApplicationLoader {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            #[cfg(linux)]
            config: YaobowConfig::load("~/.config/openpal3.toml", "OPENPAL3"),
            #[cfg(not(linux))]
            config: YaobowConfig::load("openpal3.toml", "OPENPAL3"),
            selected_game: Rc::new(RefCell::new(None)),
        }
    }
}

fn create_loader(
    game: GameType,
    app: ComRc<IApplication>,
    config: YaobowConfig,
) -> ComRc<IApplicationLoaderComponent> {
    match game {
        GameType::PAL3 => OpenPal3ApplicationLoader::create(app, &config),
        GameType::PAL4 => OpenPal4ApplicationLoader::create(app, config),
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
    app.run();
}
