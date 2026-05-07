//! Scripted replacement for the legacy Rust welcome director.
//!
//! Renders welcome.p7 via radiance_scripting; routes button commands to either
//! per-game MainPageDirector construction, settings page, or full script reload.

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

use crate::directors::config_service::ConfigService;
use crate::GameType;
use crate::SceneViewResourceView;

const WELCOME_SCRIPT: &str = include_str!("../../scripts/welcome.p7");
const SETTINGS_SCRIPT: &str = include_str!("../../scripts/settings.p7");

const RELOAD_CMD: i32 = 9999;
const SETTINGS_CMD: i32 = 9001;
const SAVE_CMD: i32 = 9002;
const CANCEL_CMD: i32 = 9000;
const GAME_CMD_BASE: i32 = 1000;
const PICK_CMD_BASE: i32 = 4000;
const CLEAR_CMD_BASE: i32 = 5000;

pub struct ScriptedWelcomePage;

impl ScriptedWelcomePage {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        let config = Rc::new(RefCell::new(YaobowConfig::load()));
        Self::create_with_config(app, config)
    }

    fn create_with_config(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
    ) -> ComRc<IDirector> {
        let host_ctx = build_host_context(&app, config.clone());
        let ui_mgr = app.engine().borrow().ui_manager();
        let textures = build_texture_cache(&app);
        let router = Rc::new(WelcomeCommandRouter::new(app.clone(), config));
        ScriptedDirector::create_with_ui(WELCOME_SCRIPT, host_ctx, ui_mgr, textures, router)
            .expect("welcome.p7 must load successfully")
    }

    fn create_settings(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
    ) -> ComRc<IDirector> {
        let host_ctx = build_host_context(&app, config.clone());
        let ui_mgr = app.engine().borrow().ui_manager();
        let textures = build_texture_cache(&app);
        let router = Rc::new(SettingsCommandRouter::new(app.clone(), config));
        ScriptedDirector::create_with_ui(SETTINGS_SCRIPT, host_ctx, ui_mgr, textures, router)
            .expect("settings.p7 must load successfully")
    }
}

fn build_host_context(
    app: &ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
) -> ComRc<IHostContext> {
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
        ConfigService::create(config),
    )
}

fn build_texture_cache(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    Rc::new(RefCell::new(ImguiTextureCache::new(factory)))
}

pub struct WelcomeCommandRouter {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
}

impl WelcomeCommandRouter {
    pub fn new(app: ComRc<IApplication>, config: Rc<RefCell<YaobowConfig>>) -> Self {
        Self { app, config }
    }

    fn load_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        let game = match game_from_ordinal(ordinal) {
            Some(game) => game,
            None => {
                log::warn!("unknown welcome game ordinal: {}", ordinal);
                return None;
            }
        };

        let asset_path = self.config.borrow().asset_path_for(game).to_string();
        if asset_path.is_empty() {
            log::warn!(
                "no asset path configured for {}; routing to settings",
                game.full_name()
            );
            return Some(ScriptedWelcomePage::create_settings(
                self.app.clone(),
                self.config.clone(),
            ));
        }

        let resource_view_content =
            SceneViewResourceView::new(&asset_path, self.app.clone(), game);
        let plugins = SceneViewPlugins::new(Some(ComRc::from_object(resource_view_content)));

        let input = self.app.engine().borrow().input_engine().clone();
        let ui = self.app.engine().borrow().ui_manager();
        let scene_manager = self.app.engine().borrow().scene_manager();
        Some(MainPageDirector::create(Some(plugins), ui, input, scene_manager))
    }

    fn reload_script(&self) -> Option<ComRc<IDirector>> {
        Some(ScriptedWelcomePage::create_with_config(
            self.app.clone(),
            self.config.clone(),
        ))
    }

    fn open_settings(&self) -> Option<ComRc<IDirector>> {
        Some(ScriptedWelcomePage::create_settings(
            self.app.clone(),
            self.config.clone(),
        ))
    }
}

impl CommandRouter for WelcomeCommandRouter {
    fn dispatch(&self, command_id: i32) -> Option<ComRc<IDirector>> {
        log::info!("WelcomeCommandRouter: dispatching cmd {}", command_id);
        if command_id == RELOAD_CMD {
            return self.reload_script();
        }
        if command_id == SETTINGS_CMD {
            return self.open_settings();
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

pub struct SettingsCommandRouter {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
}

impl SettingsCommandRouter {
    pub fn new(app: ComRc<IApplication>, config: Rc<RefCell<YaobowConfig>>) -> Self {
        Self { app, config }
    }
}

impl CommandRouter for SettingsCommandRouter {
    fn dispatch(&self, command_id: i32) -> Option<ComRc<IDirector>> {
        log::info!("SettingsCommandRouter: dispatching cmd {}", command_id);

        if command_id == SAVE_CMD {
            if let Err(e) = self.config.borrow().save() {
                log::warn!("settings save failed: {}", e);
            }
            return Some(ScriptedWelcomePage::create_with_config(
                self.app.clone(),
                self.config.clone(),
            ));
        }

        if command_id == CANCEL_CMD {
            // Discard unsaved edits.
            *self.config.borrow_mut() = YaobowConfig::load();
            return Some(ScriptedWelcomePage::create_with_config(
                self.app.clone(),
                self.config.clone(),
            ));
        }

        if (PICK_CMD_BASE..PICK_CMD_BASE + 1000).contains(&command_id) {
            let ordinal = command_id - PICK_CMD_BASE;
            if let Some(game) = game_from_ordinal(ordinal) {
                let initial = self.config.borrow().asset_path_for(game).to_string();
                let picked = pick_folder(&initial);
                if !picked.is_empty() {
                    self.config.borrow_mut().set_asset_path(game, picked);
                }
            }
            return None;
        }

        if (CLEAR_CMD_BASE..CLEAR_CMD_BASE + 1000).contains(&command_id) {
            let ordinal = command_id - CLEAR_CMD_BASE;
            if let Some(game) = game_from_ordinal(ordinal) {
                self.config
                    .borrow_mut()
                    .set_asset_path(game, String::new());
            }
            return None;
        }

        None
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn pick_folder(initial: &str) -> String {
    use native_dialog::FileDialogBuilder;
    let mut dialog = FileDialogBuilder::default();
    if !initial.is_empty() {
        dialog = dialog.set_location(initial);
    }
    match dialog.open_single_dir().show() {
        Ok(Some(path)) => path.to_string_lossy().to_string(),
        Ok(None) => String::new(),
        Err(e) => {
            log::warn!("folder picker failed: {}", e);
            String::new()
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pick_folder(_initial: &str) -> String {
    String::new()
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
