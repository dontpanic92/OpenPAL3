mod director;

use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent, IComponent, IComponentImpl},
    scene::CoreScene,
};
use shared::config::YaobowConfig;

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

        let director = TitleSelectionDirector::new(self.app.clone(), self.selected_game.clone());
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        scene_manager.set_director(ComRc::from_object(director));
        scene_manager.push_scene(CoreScene::create());
    }

    fn on_updating(&self, delta_sec: f32) {
        if self.selected_game.borrow().is_none() {
            return;
        }

        let game = self.selected_game.borrow().unwrap();
        let loader = game
            .create_loader(self.app.clone(), self.config.as_ref().cloned().unwrap())
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
            config: YaobowConfig::load("openpal3.toml", "OPENPAL3"),
            selected_game: Rc::new(RefCell::new(None)),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum GameType {
    PAL3,
    PAL3A,
    PAL4,
    PAL5,
    PAL5Q,
    SWD5,
    SWDHC,
    SWDCF,
    Gujian,
    Gujian2,
}

impl GameType {
    pub fn app_name(&self) -> &'static str {
        match self {
            GameType::PAL3 => "OpenPAL3",
            GameType::PAL3A => "OpenPAL3A",
            GameType::PAL4 => "OpenPAL4",
            GameType::PAL5 => "OpenPAL5",
            GameType::PAL5Q => "OpenPAL5Q",
            GameType::SWD5 => "OpenSWD5",
            GameType::SWDHC => "OpenSWDHC",
            GameType::SWDCF => "OpenSWDCF",
            GameType::Gujian => "OpenGujian",
            GameType::Gujian2 => "OpenGujian2",
        }
    }

    pub fn full_name(&self) -> &'static str {
        match self {
            GameType::PAL3 => "仙剑奇侠传三",
            GameType::PAL3A => "仙剑奇侠传三外传",
            GameType::PAL4 => "仙剑奇侠传四",
            GameType::PAL5 => "仙剑奇侠传五",
            GameType::PAL5Q => "仙剑奇侠传五前传",
            GameType::SWD5 => "轩辕剑五",
            GameType::SWDHC => "轩辕剑外传 汉之云",
            GameType::SWDCF => "轩辕剑外传 云之遥",
            GameType::Gujian => "古剑奇谭",
            GameType::Gujian2 => "古剑奇谭二",
        }
    }

    fn create_loader(
        &self,
        app: ComRc<IApplication>,
        config: YaobowConfig,
    ) -> ComRc<IApplicationLoaderComponent> {
        match self {
            GameType::PAL3 => OpenPal3ApplicationLoader::create(app, &config),
            GameType::PAL4 => OpenPal4ApplicationLoader::create(app, config),
            _ => unimplemented!(),
        }
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
