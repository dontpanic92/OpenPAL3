//! Implementation of `IAppService` exposed to p7 scripts.
//!
//! The editor's only host-built director transition: load a game's asset
//! tree and return the editor's `MainPageDirector` (Rust-implemented)
//! ready for the script to wrap in its `HostDirector` adapter.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::comdef::{IApplication, IDirector};
use radiance_editor::{director::MainPageDirector, ui::scene_view::SceneViewPlugins};
use radiance_scripting::comdef::services::{IAppService, IAppServiceImpl};
use radiance_scripting::ScriptHost;
use shared::config::YaobowConfig;

use crate::GameType;
use crate::SceneViewResourceView;

pub struct AppService {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    script_host: Rc<ScriptHost>,
}

radiance_scripting::ComObject_AppService!(super::AppService);

impl AppService {
    pub fn create(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
        script_host: Rc<ScriptHost>,
    ) -> ComRc<IAppService> {
        ComRc::from_object(Self {
            app,
            config,
            script_host,
        })
    }
}

impl IAppServiceImpl for AppService {
    fn open_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        let game = game_from_ordinal(ordinal)?;
        let asset_path = self.config.borrow().asset_path_for(game).to_string();
        if asset_path.is_empty() {
            log::warn!(
                "no asset path configured for {}; script should route to settings",
                game.full_name()
            );
            return None;
        }

        let resource_view_content = SceneViewResourceView::new(
            &asset_path,
            self.app.clone(),
            game,
            self.script_host.clone(),
        );
        let plugins = SceneViewPlugins::new(Some(ComRc::from_object(resource_view_content)));

        let engine = self.app.engine();
        let engine = engine.borrow();
        Some(MainPageDirector::create(
            Some(plugins),
            engine.ui_manager(),
            engine.input_engine(),
            engine.scene_manager(),
        ))
    }
}

fn game_from_ordinal(ordinal: i32) -> Option<GameType> {
    match ordinal {
        0 => Some(GameType::PAL3),
        1 => Some(GameType::PAL3A),
        2 => Some(GameType::PAL4),
        3 => Some(GameType::PAL5),
        4 => Some(GameType::PAL5Q),
        5 => Some(GameType::SWD5),
        6 => Some(GameType::SWDHC),
        7 => Some(GameType::SWDCF),
        8 => Some(GameType::Gujian),
        9 => Some(GameType::Gujian2),
        _ => None,
    }
}

/// Builds the default `IHostContext` for the editor that:
/// - uses the editor's `ConfigService` and `AppService`,
/// - exposes the radiance engine's scene/audio/input services,
/// - provides an empty VFS until a game is opened.
pub fn build_host_context(
    app: &ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    script_host: Rc<ScriptHost>,
) -> ComRc<radiance_scripting::comdef::services::IHostContext> {
    use crate::directors::config_service::ConfigService;

    let engine = app.engine();
    let engine = engine.borrow();

    let app_service = AppService::create(app.clone(), config.clone(), script_host);

    radiance_scripting::services::HostContext::create(
        engine.scene_manager(),
        engine.audio_engine(),
        engine.rendering_component_factory(),
        Rc::new(MiniFs::new(false)),
        engine.input_engine(),
        app_service,
        ConfigService::create(config),
    )
}
