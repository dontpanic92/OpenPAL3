pub mod audio;
pub mod game_registry;
pub mod input;
pub mod texture;
pub mod texture_cache;
pub mod texture_resolver;
pub mod ui_host;
pub mod ui_host_recording;
pub mod vfs;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::audio::AudioEngine;
use radiance::comdef::ISceneManager;
use radiance::input::InputEngine;
use radiance::rendering::ComponentFactory;

use crate::comdef::services::{
    IAppService, IAudioService, IConfigService, IGameRegistry, IHostContext, IHostContextImpl,
    IInputService, ITextureService, IVfsService,
};

pub struct HostContext {
    scene_manager: ComRc<ISceneManager>,
    audio: ComRc<IAudioService>,
    textures: ComRc<ITextureService>,
    vfs: ComRc<IVfsService>,
    input: ComRc<IInputService>,
    games: ComRc<IGameRegistry>,
    app: ComRc<IAppService>,
    config: ComRc<IConfigService>,
}

ComObject_HostContext!(super::HostContext);

impl HostContext {
    pub fn new(
        scene_manager: ComRc<ISceneManager>,
        audio: ComRc<IAudioService>,
        textures: ComRc<ITextureService>,
        vfs: ComRc<IVfsService>,
        input: ComRc<IInputService>,
        games: ComRc<IGameRegistry>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
    ) -> ComRc<IHostContext> {
        ComRc::from_object(Self {
            scene_manager,
            audio,
            textures,
            vfs,
            input,
            games,
            app,
            config,
        })
    }

    pub fn create(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        texture_factory: Rc<dyn ComponentFactory>,
        vfs: Rc<MiniFs>,
        input: Rc<RefCell<dyn InputEngine>>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
    ) -> ComRc<IHostContext> {
        Self::new(
            scene_manager,
            AudioService::create(audio_engine, vfs.clone()),
            TextureService::create(texture_factory, vfs.clone()),
            VfsService::create(vfs),
            InputService::create(input),
            GameRegistry::create(),
            app,
            config,
        )
    }
}

impl IHostContextImpl for HostContext {
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
    fn config(&self) -> ComRc<IConfigService> {
        self.config.clone()
    }
}

pub use audio::{AudioService, AudioSource};
pub use game_registry::GameRegistry;
pub use input::InputService;
pub use texture::{Texture, TextureService};
pub use texture_cache::ImguiTextureCache;
pub use vfs::VfsService;
