//! SWD5-family launch service — phase-2 replacement for
//! `Swd5LaunchContext`. Mirrors the `Pal4Service` shape exactly: an
//! app-lifetime singleton exposed via `IYaobowHostContext.swd5()` that
//! returns a fully-wired `IDirector` for the requested game.
//!
//! `create_director(asset_path, game_ordinal)` accepts the per-game
//! ordinal because SWD5 / SWDHC / SWDCF share infrastructure but feed
//! the asset loader different `GameType` discriminators (texture
//! resolver branch, asset-table selection).

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance::scene::CoreScene;

use crate::openswd5::asset_loader::AssetLoader;
use crate::openswd5::comdef::{ISwd5Service, ISwd5ServiceImpl};
use crate::openswd5::director::OpenSWD5Director;
use crate::GameType;

pub struct Swd5Service {
    app: ComRc<IApplication>,
}

ComObject_Swd5Service!(super::Swd5Service);

impl Swd5Service {
    pub fn create(app: ComRc<IApplication>) -> ComRc<ISwd5Service> {
        ComRc::from_object(Self { app })
    }
}

impl ISwd5ServiceImpl for Swd5Service {
    fn create_director(
        &self,
        asset_path: &str,
        game_ordinal: std::os::raw::c_int,
    ) -> ComRc<IDirector> {
        let game = radiance_scripting::services::game_registry::ordinal_to_config_key(
            game_ordinal as i32,
        )
        .and_then(GameType::from_config_key)
        .unwrap_or(GameType::SWDHC);

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        let input_engine = engine.input_engine();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);

        let asset_path = PathBuf::from(asset_path);
        let vfs = init_virtual_fs(asset_path.to_str().unwrap_or(""), None);
        let loader = AssetLoader::new(component_factory.clone(), Rc::new(vfs), game);

        // Push an empty initial scene so the Lua VM's first tick sees
        // a valid scene-manager state. Matches today's loader.
        scene_manager.push_scene(CoreScene::create());

        let director = OpenSWD5Director::new(
            loader,
            input_engine,
            scene_manager,
            audio_engine,
            component_factory,
            ui,
        );

        ComRc::<IDirector>::from_object(director)
    }
}
