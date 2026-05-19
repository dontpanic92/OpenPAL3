//! App-lifetime host context for the yaobow protosept runtime.
//!
//! Wires `radiance_scripting`'s shared services (audio / textures /
//! vfs / input / games / random) against the yaobow app asset VFS.
//! This object is passed once to `app.p7:init(ctx)` and then reused by
//! script-owned feature modules.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{LocalFs, MiniFs, ZipFs};
use radiance::comdef::{IApplication, IDirector, ISceneManager};
use radiance_scripting::comdef::services::{
    IAppService, IAppServiceImpl, IAudioService, IGameRegistry, IHostContextImpl, IInputService,
    IRandomService, ITextureService, IVfsService,
};
use radiance_scripting::services::{
    AudioService, GameRegistry, InputService, RandomService, TextureService, VfsService,
};
use shared::GameType;

use crate::comdef::yaobow_services::{IYaobowAppContext, IYaobowAppContextImpl};

pub struct YaobowAppContext {
    scene_manager: ComRc<ISceneManager>,
    audio: ComRc<IAudioService>,
    textures: ComRc<ITextureService>,
    vfs: ComRc<IVfsService>,
    input: ComRc<IInputService>,
    games: ComRc<IGameRegistry>,
    app: ComRc<IAppService>,
    random: ComRc<IRandomService>,
}

ComObject_YaobowAppContext!(super::YaobowAppContext);

impl YaobowAppContext {
    pub fn create(
        app: ComRc<IApplication>,
        selected_game: Rc<RefCell<Option<GameType>>>,
    ) -> ComRc<IYaobowAppContext> {
        let engine_rc = app.engine();
        let engine = engine_rc.borrow();
        let vfs = load_app_vfs();
        let app_service: ComRc<IAppService> = YaobowAppService::create(selected_game);

        ComRc::from_object(Self {
            scene_manager: engine.scene_manager(),
            audio: AudioService::create(engine.audio_engine(), vfs.clone()),
            textures: TextureService::create(engine.rendering_component_factory(), vfs.clone()),
            vfs: VfsService::create(vfs),
            input: InputService::create(engine.input_engine()),
            games: GameRegistry::create(),
            app: app_service,
            random: RandomService::create(),
        })
    }
}

impl IHostContextImpl for YaobowAppContext {
    fn scene_manager(&self) -> ComRc<ISceneManager> {
        self.scene_manager.clone()
    }
    fn audio(&self) -> ComRc<IAudioService> {
        self.audio.clone()
    }
    fn textures(&self) -> ComRc<ITextureService> {
        self.textures.clone()
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        self.vfs.clone()
    }
    fn input(&self) -> ComRc<IInputService> {
        self.input.clone()
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        self.games.clone()
    }
    fn app(&self) -> ComRc<IAppService> {
        self.app.clone()
    }
    fn random(&self) -> ComRc<IRandomService> {
        self.random.clone()
    }
}

impl IYaobowAppContextImpl for YaobowAppContext {}

// ---------------------------------------------------------------------------
// `IAppService` impl. `open_game(ordinal)` writes the shared selected-game
// slot that `YaobowApplicationLoader::on_updating` polls; returning `None`
// keeps every game swap on the Rust side (the title director's `update`
// returns `null`, the engine runs one more frame, `on_updating` swaps in
// PAL3's / PAL4's loader). This avoids duplicating loader-selection logic
// on the script side.

pub struct YaobowAppService {
    selected_game: Rc<RefCell<Option<GameType>>>,
}

radiance_scripting::ComObject_AppService!(super::YaobowAppService);

impl YaobowAppService {
    pub fn create(selected_game: Rc<RefCell<Option<GameType>>>) -> ComRc<IAppService> {
        ComRc::from_object(Self { selected_game })
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
}

fn game_from_ordinal(ordinal: i32) -> Option<GameType> {
    // Mirrors `GAMES` in the legacy `application/director.rs` plus
    // the constants in `application/scripts/title_consts.p7`. Only
    // PAL3 and PAL4 are wired up today.
    match ordinal {
        0 => Some(GameType::PAL3),
        1 => Some(GameType::PAL4),
        _ => None,
    }
}

/// Probes for the yaobow asset zip in the same order as the legacy
/// `TitleSelectionDirector::load_vfs`: bundled zip, dev tree, parent
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
