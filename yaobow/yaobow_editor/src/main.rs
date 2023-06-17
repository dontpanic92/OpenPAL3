#![feature(drain_filter)]
mod comdef;
mod config;
mod directors;
mod preview;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use directors::welcome_page::WelcomePageDirector;
use directors::DevToolsDirector;
use radiance::application::Application;
use radiance::comdef::{IApplication, IApplicationLoaderComponent, IDirector, ISceneManager};
use radiance_editor::application::EditorApplicationLoader;
use radiance_editor::comdef::IViewContentImpl;
use shared::config::YaobowConfig;
use shared::openpal3::asset_manager::AssetManager;

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
}

pub struct SceneViewResourceView {
    ui: RefCell<Option<ComRc<IDirector>>>,
}

ComObject_YaobowResourceViewContent!(crate::SceneViewResourceView);

impl IViewContentImpl for SceneViewResourceView {
    fn render(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void {
        let mut director = self.ui.borrow_mut();
        let view = director.as_mut().unwrap();
        view.update(scene_manager, ui, delta_sec);
    }
}

impl SceneViewResourceView {
    pub fn new(config: YaobowConfig, app: ComRc<IApplication>, game: GameType) -> Self {
        app.set_title(&format!("妖弓编辑器 - {}", game.app_name()));

        let pkg_key = match game {
            GameType::PAL5 => Some("Y%H^uz6i"),
            GameType::PAL5Q => Some("L#Z^zyjq"),
            _ => None,
        };

        let factory = app.engine().borrow().rendering_component_factory();
        let asset_mgr = AssetManager::new(factory, &config.asset_path, pkg_key);
        let audio_engine = app.engine().borrow().audio_engine();
        let ui = Some(DevToolsDirector::new(
            audio_engine,
            Rc::new(asset_mgr),
            game,
        ));

        SceneViewResourceView {
            ui: RefCell::new(ui),
        }
    }
}

fn main() {
    let logger = simple_logger::SimpleLogger::new();

    // workaround panic on Linux for 'Could not determine the UTC offset on this system'
    // see: https://github.com/borntyping/rust-simple_logger/issues/47
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();

    logger.init().unwrap();

    // let mut line = String::new();
    // let stdin = std::io::stdin();
    // stdin.lock().read_line(&mut line).unwrap();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        let game = match args[1].as_str() {
            "--pal4" => GameType::PAL4,
            "--pal5" => GameType::PAL5,
            "--pal5q" => GameType::PAL5Q,
            "--swd5" => GameType::SWD5,
            "--swdhc" => GameType::SWDHC,
            "--swdcf" => GameType::SWDCF,
            "--gujian" => GameType::Gujian,
            "--gujian2" => GameType::Gujian2,
            &_ => GameType::PAL3,
        };
    }

    let app = ComRc::<IApplication>::from_object(Application::new());
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(EditorApplicationLoader::new(
            app.clone(),
            WelcomePageDirector::create(app.clone()),
        )),
    );

    config::init_imgui_ini(&app);

    app.initialize();
    app.run();
}
