use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IDirector};
use radiance_scripting::comdef::services::{IAppService, IAppServiceImpl};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::{ScriptHost, ScriptedDirector};
use shared::config::YaobowConfig;

use crate::directors::config_service::ConfigService;
use crate::directors::DevToolsAssetLoader;
use crate::services::editor_host_context::EditorHostContext;
use crate::GameType;

pub struct AppService {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    script_host: Rc<ScriptHost>,
    textures: Rc<RefCell<ImguiTextureCache>>,
}

radiance_scripting::ComObject_AppService!(super::AppService);

impl AppService {
    pub fn create(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
        script_host: Rc<ScriptHost>,
    ) -> ComRc<IAppService> {
        let factory = app.engine().borrow().rendering_component_factory();
        let textures = Rc::new(RefCell::new(ImguiTextureCache::new(factory)));
        ComRc::from_object(Self {
            app,
            config,
            script_host,
            textures,
        })
    }

    /// Variant used by the welcome bootstrap so the welcome page and the
    /// later main editor share the same imgui texture cache (otherwise
    /// previewer com_ids registered by `IPreviewerHub` would not resolve).
    pub fn create_with_textures(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
        script_host: Rc<ScriptHost>,
        textures: Rc<RefCell<ImguiTextureCache>>,
    ) -> ComRc<IAppService> {
        ComRc::from_object(Self {
            app,
            config,
            script_host,
            textures,
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

        // Build a per-game vfs and asset loader.
        let pkg_key = match game {
            GameType::PAL5 => Some("Y%H^uz6i"),
            GameType::PAL5Q => Some("L#Z^zyjq"),
            _ => None,
        };
        let factory = self.app.engine().borrow().rendering_component_factory();
        let raw_vfs = packfs::init_virtual_fs(&asset_path, pkg_key);
        let asset_loader = match game {
            GameType::PAL4 => DevToolsAssetLoader::Pal4(
                shared::openpal4::asset_loader::AssetLoader::new(
                    factory.clone(),
                    self.app.engine().borrow().input_engine(),
                    raw_vfs,
                ),
            ),
            GameType::PAL5 => DevToolsAssetLoader::Pal5(
                shared::openpal5::asset_loader::AssetLoader::new(factory.clone(), Rc::new(raw_vfs)),
            ),
            GameType::SWD5 | GameType::SWDHC | GameType::SWDCF => DevToolsAssetLoader::Swd5(
                shared::openswd5::asset_loader::AssetLoader::new(
                    factory.clone(),
                    Rc::new(raw_vfs),
                    game,
                ),
            ),
            _ => DevToolsAssetLoader::Pal3(Rc::new(
                shared::openpal3::asset_manager::AssetManager::new(factory.clone(), Rc::new(raw_vfs)),
            )),
        };
        let vfs_rc = asset_loader.vfs_rc();

        self.app.set_title(&format!("妖弓编辑器 - {}", game.app_name()));

        let engine = self.app.engine();
        let engine = engine.borrow();

        // Build a fresh editor host context bound to the opened game's vfs
        // and assets.
        let editor_app_service = AppService::create_with_textures(
            self.app.clone(),
            self.config.clone(),
            self.script_host.clone(),
            self.textures.clone(),
        );
        let cfg_service = ConfigService::create(self.config.clone());

        let host_ctx = EditorHostContext::create_for_game(
            engine.scene_manager(),
            engine.audio_engine(),
            factory.clone(),
            engine.input_engine(),
            editor_app_service,
            cfg_service,
            self.textures.clone(),
            vfs_rc,
            asset_loader,
            game,
        );

        let host_id = self.script_host.intern(host_ctx);
        let host_box = self
            .script_host
            .foreign_box(
                "yaobow_editor.comdef.editor_services.IEditorHostContext",
                host_id,
            )
            .ok()?;
        let director_data = self
            .script_host
            .call_returning_data("init_main_editor", vec![host_box])
            .ok()?;
        let handle = self.script_host.root(director_data);

        Some(ScriptedDirector::with_ui(
            self.script_host.clone(),
            handle,
            engine.ui_manager(),
            self.textures.clone(),
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
