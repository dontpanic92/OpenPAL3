#![feature(drain_filter)]
mod directors;

mod preview;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use directors::DevToolsDirector;
use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::Application;
use radiance::comdef::{IApplication, IApplicationLoaderComponent};
use radiance::scene::Director;
use radiance_editor::application::EditorApplicationLoader;
use radiance_editor::comdef::IViewContentImpl;
use radiance_editor::ui::scene_view::SceneViewPlugins;
use radiance_editor::ComObject_ResourceViewContent;

const TITLE: &str = "妖弓编辑器 - OpenPAL3";

pub enum GameType {
    PAL3,
    PAL4,
    PAL5,
    PAL5Q,
}

pub struct SceneViewResourceView {
    ui: RefCell<Option<Rc<RefCell<DevToolsDirector>>>>,
}

ComObject_ResourceViewContent!(crate::SceneViewResourceView);

impl IViewContentImpl for SceneViewResourceView {
    fn render(
        &self,
        scene_manager: &mut dyn radiance::scene::SceneManager,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void {
        let mut director = self.ui.borrow_mut();
        let view = director.as_mut().unwrap();
        view.borrow_mut().update(scene_manager, ui, delta_sec);
    }
}

impl SceneViewResourceView {
    pub fn new(config: OpenGbConfig, app: ComRc<IApplication>, game: GameType) -> Self {
        app.set_title(TITLE);

        let pkg_key = match game {
            GameType::PAL5 => Some("Y%H^uz6i"),
            GameType::PAL5Q => Some("L#Z^zyjq"),
            _ => None,
        };

        let factory = app.engine().borrow().rendering_component_factory();
        let asset_mgr = AssetManager::new(factory, &config.asset_path, pkg_key);
        let audio_engine = app.engine().borrow().audio_engine();
        let ui = Some(DevToolsDirector::new(audio_engine, Rc::new(asset_mgr)));

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

    let mut config = OpenGbConfig::load("openpal3.toml", "OPENPAL3");
    let mut game = GameType::PAL3;

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        match args[1].as_str() {
            "--pal4" => {
                config.asset_path = "F:\\PAL4\\".to_string();
                game = GameType::PAL4;
            }
            "--pal5" => {
                config.asset_path = "F:\\PAL5\\".to_string();
                game = GameType::PAL5;
            }
            "--pal5q" => {
                config.asset_path = "F:\\PAL5Q\\".to_string();
                game = GameType::PAL5Q;
            }
            &_ => {}
        }
    }

    let app = ComRc::<IApplication>::from_object(Application::new());
    let resource_view_content = SceneViewResourceView::new(config, app.clone(), game);
    let plugins = SceneViewPlugins::new(Some(crosscom::ComRc::from_object(resource_view_content)));

    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(EditorApplicationLoader::new(app.clone(), Some(plugins))),
    );

    app.initialize();
    app.run();
}
