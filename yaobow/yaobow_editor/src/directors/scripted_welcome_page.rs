//! Scripted replacement for the legacy Rust welcome director.
//!
//! Renders welcome.p7 via radiance_scripting; routes button commands to either
//! per-game MainPageDirector construction or full script reload.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::comdef::{IApplication, IDirector};
use radiance_scripting::comdef::services::IHostContext;
use radiance_scripting::services::{HostContext, ImguiTextureCache};
use radiance_scripting::{CommandRouter, ScriptedDirector};
use radiance_editor::{director::MainPageDirector, ui::scene_view::SceneViewPlugins};
use shared::config::YaobowConfig;

use crate::GameType;
use crate::SceneViewResourceView;

const WELCOME_SCRIPT: &str = include_str!("../../scripts/welcome.p7");
const RELOAD_CMD: i32 = 9999;
const GAME_CMD_BASE: i32 = 1000;

pub struct ScriptedWelcomePage;

impl ScriptedWelcomePage {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        let host_ctx = build_host_context(&app);
        let ui_mgr = app.engine().borrow().ui_manager();
        let textures = build_texture_cache(&app);
        let router = Rc::new(WelcomeCommandRouter::new(app.clone()));
        ScriptedDirector::create_with_ui(WELCOME_SCRIPT, host_ctx, ui_mgr, textures, router)
            .expect("welcome.p7 must load successfully")
    }
}

fn build_host_context(app: &ComRc<IApplication>) -> ComRc<IHostContext> {
    let engine = app.engine();
    let engine = engine.borrow();
    let scene_manager = engine.scene_manager();
    let audio_engine = engine.audio_engine();
    let texture_factory = engine.rendering_component_factory();
    let input = engine.input_engine();

    // The welcome script only queries game metadata. Per-game VFS instances are
    // constructed lazily by the router once a game button is selected.
    HostContext::create(
        scene_manager,
        audio_engine,
        texture_factory,
        Rc::new(MiniFs::new(false)),
        input,
    )
}

fn build_texture_cache(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    Rc::new(RefCell::new(ImguiTextureCache::new(factory)))
}

pub struct WelcomeCommandRouter {
    app: ComRc<IApplication>,
}

impl WelcomeCommandRouter {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self { app }
    }

    fn load_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        let game = match game_from_ordinal(ordinal) {
            Some(game) => game,
            None => {
                log::warn!("unknown welcome game ordinal: {}", ordinal);
                return None;
            }
        };

        let mut config = YaobowConfig::load("openpal3.toml", "OpenPAL3").unwrap();
        match game {
            GameType::PAL3A => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\PAL3A".to_string();
            }
            GameType::PAL4 => {
                config.asset_path =
                    "F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 4\\".to_string();
            }
            GameType::PAL5 => {
                config.asset_path = "F:\\PAL5\\".to_string();
            }
            GameType::PAL5Q => {
                config.asset_path = "F:\\PAL5Q\\".to_string();
            }
            GameType::SWD5 => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWD5".to_string();
            }
            GameType::SWDHC => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWDHC".to_string();
            }
            GameType::SWDCF => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWDCF".to_string();
            }
            GameType::Gujian => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\Gujian".to_string();
            }
            GameType::Gujian2 => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\Gujian2".to_string();
            }
            _ => {}
        };

        let resource_view_content = SceneViewResourceView::new(config, self.app.clone(), game);
        let plugins = SceneViewPlugins::new(Some(ComRc::from_object(resource_view_content)));

        let input = self.app.engine().borrow().input_engine().clone();
        let ui = self.app.engine().borrow().ui_manager();
        let scene_manager = self.app.engine().borrow().scene_manager();
        Some(MainPageDirector::create(Some(plugins), ui, input, scene_manager))
    }

    fn reload_script(&self) -> Option<ComRc<IDirector>> {
        Some(ScriptedWelcomePage::create(self.app.clone()))
    }
}

impl CommandRouter for WelcomeCommandRouter {
    fn dispatch(&self, command_id: i32) -> Option<ComRc<IDirector>> {
        log::info!("WelcomeCommandRouter: dispatching cmd {}", command_id);
        if command_id == RELOAD_CMD {
            return self.reload_script();
        }
        if (GAME_CMD_BASE..GAME_CMD_BASE + 1000).contains(&command_id) {
            let next = self.load_game(command_id - GAME_CMD_BASE);
            log::info!(
                "WelcomeCommandRouter: load_game({}) -> {}",
                command_id - GAME_CMD_BASE,
                if next.is_some() { "Some" } else { "None" }
            );
            return next;
        }
        None
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
