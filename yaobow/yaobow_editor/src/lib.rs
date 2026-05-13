#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/yaobow_editor_comdef.rs"));

    // Mirror radiance_scripting's services namespace so cross-crate uses of
    // `radiance_scripting::ComObject_*!` macros (which expand `use crate as
    // radiance_scripting`) can resolve `radiance_scripting::comdef::services::*`
    // through this crate.
    pub mod services {
        pub use radiance_scripting::comdef::services::*;
    }
}

pub mod config;
pub mod directors;
pub mod preview;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IDirector};
use radiance_editor::comdef::IViewContentImpl;
use radiance_scripting::ScriptHost;
use shared::openpal3::asset_manager::AssetManager;
pub use shared::GameType;

use directors::{DevToolsAssetLoader, DevToolsDirector};

pub struct SceneViewResourceView {
    director: RefCell<Option<ComRc<IDirector>>>,
}

ComObject_YaobowResourceViewContent!(crate::SceneViewResourceView);

impl IViewContentImpl for SceneViewResourceView {
    fn render(&self, delta_sec: f32) -> crosscom::Void {
        let mut director = self.director.borrow_mut();
        let view = director.as_mut().unwrap();
        view.update(delta_sec);
    }
}

impl SceneViewResourceView {
    pub fn new(
        asset_path: &str,
        app: ComRc<IApplication>,
        game: GameType,
        script_runtime: Rc<ScriptHost>,
    ) -> Self {
        app.set_title(&format!("妖弓编辑器 - {}", game.app_name()));

        let pkg_key = match game {
            GameType::PAL5 => Some("Y%H^uz6i"),
            GameType::PAL5Q => Some("L#Z^zyjq"),
            _ => None,
        };

        let factory = app.engine().borrow().rendering_component_factory();
        let input = app.engine().borrow().input_engine();
        let vfs = packfs::init_virtual_fs(asset_path, pkg_key);
        let asset_loader = match game {
            GameType::PAL4 => {
                DevToolsAssetLoader::Pal4(shared::openpal4::asset_loader::AssetLoader::new(
                    factory.clone(),
                    input.clone(),
                    vfs,
                ))
            }
            GameType::PAL5 => DevToolsAssetLoader::Pal5(
                shared::openpal5::asset_loader::AssetLoader::new(factory.clone(), Rc::new(vfs)),
            ),
            GameType::SWD5 => {
                DevToolsAssetLoader::Swd5(shared::openswd5::asset_loader::AssetLoader::new(
                    factory.clone(),
                    Rc::new(vfs),
                    game,
                ))
            }
            _ => {
                DevToolsAssetLoader::Pal3(Rc::new(AssetManager::new(factory.clone(), Rc::new(vfs))))
            }
        };

        let audio_engine = app.engine().borrow().audio_engine();
        let scene_manager = app.engine().borrow().scene_manager();
        let ui = app.engine().borrow().ui_manager();
        let director = Some(DevToolsDirector::new(
            audio_engine,
            scene_manager,
            asset_loader,
            ui,
            game,
            script_runtime,
        ));

        SceneViewResourceView {
            director: RefCell::new(director),
        }
    }
}
