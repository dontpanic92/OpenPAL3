//! App-lifetime host context for the yaobow protosept runtime.
//!
//! Constructs the canonical `IHostContext` from `radiance_scripting`
//! and wires in the yaobow-specific app VFS + `IAppService`. There's
//! no per-app `IYaobowAppContext` interface anymore — scripts get the
//! canonical surface.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{LocalFs, MiniFs, ZipFs};
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance_scripting::comdef::services::{IAppService, IAppServiceImpl, IHostContext};
use radiance_scripting::services::HostContext;
use shared::config::YaobowConfig;
use shared::config_service::ConfigService;
use shared::GameType;

pub struct YaobowAppContext;

impl YaobowAppContext {
    pub fn create(
        app: ComRc<IApplication>,
        selected_game: Rc<RefCell<Option<GameType>>>,
        config: Rc<RefCell<YaobowConfig>>,
    ) -> ComRc<IHostContext> {
        let engine_rc = app.engine();
        let engine = engine_rc.borrow();
        let vfs = load_app_vfs();
        let app_service: ComRc<IAppService> = YaobowAppService::create(app.clone(), selected_game);
        let imgui_ctx = engine.ui_manager().imgui_context();
        let config_service =
            ConfigService::create_with_imgui(config, Some(imgui_ctx));

        HostContext::create(
            engine.scene_manager(),
            engine.audio_engine(),
            engine.rendering_component_factory(),
            vfs,
            engine.input_engine(),
            app_service,
            config_service,
        )
    }
}

pub struct YaobowAppService {
    app: ComRc<IApplication>,
    selected_game: Rc<RefCell<Option<GameType>>>,
}

radiance_scripting::ComObject_AppService!(super::YaobowAppService);

impl YaobowAppService {
    pub fn create(
        app: ComRc<IApplication>,
        selected_game: Rc<RefCell<Option<GameType>>>,
    ) -> ComRc<IAppService> {
        ComRc::from_object(Self { app, selected_game })
    }
}

impl IAppServiceImpl for YaobowAppService {
    fn open_game(&self, ordinal: i32) -> Option<ComRc<IDirector>> {
        if let Some(game) = game_from_ordinal(ordinal) {
            self.selected_game.replace(Some(game));
        } else {
            log::warn!(
                "YaobowAppService::open_game ignoring unknown ordinal {}",
                ordinal
            );
        }
        // Returning None defers the director swap to
        // `YaobowApplicationLoader::on_updating`, which constructs
        // the per-game loader on the next tick.
        None
    }

    fn exit(&self) {
        self.app.request_exit();
    }
}

fn game_from_ordinal(ordinal: i32) -> Option<GameType> {
    radiance_scripting::services::game_registry::ordinal_to_config_key(ordinal)
        .and_then(GameType::from_config_key)
}

/// Probes for the yaobow asset zip in the same order as the legacy
/// title director's `load_vfs`: bundled zip, dev tree, parent
/// dev tree, then the FHS install location. Returns an empty `MiniFs`
/// if none match.
fn load_app_vfs() -> Rc<MiniFs> {
    let mut vfs = MiniFs::new(false);
    let zip = PathBuf::from(ASSET_PATH);
    let local1 = PathBuf::from("./yaobow/yaobow-assets");
    let local2 = PathBuf::from("../yaobow-assets");
    let local3 = PathBuf::from("/usr/share/yaobow/yaobow-assets");

    if Path::exists(&zip) {
        let local = ZipFs::new(std::fs::File::open(zip).unwrap());
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local1) {
        let local = LocalFs::new(&local1);
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local2) {
        let local = LocalFs::new(&local2);
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local3) {
        let local = LocalFs::new(&local3);
        vfs = vfs.mount(PathBuf::from("/"), local);
    }
    Rc::new(vfs)
}

#[cfg(windows)]
const ASSET_PATH: &str = "./yaobow-assets.zip";
#[cfg(any(linux, macos))]
const ASSET_PATH: &str = "../shared/yaobow/yaobow-assets.zip";
#[cfg(vita)]
const ASSET_PATH: &str = "ux0:data/yaobow-assets.zip";
#[cfg(not(any(windows, linux, macos, vita)))]
const ASSET_PATH: &str = "./yaobow-assets.zip";
