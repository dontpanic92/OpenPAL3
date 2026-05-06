pub mod audio;
pub mod command_bus;
pub mod game_registry;
pub mod input;
pub mod texture;
pub mod texture_cache;
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
    IAudioService, ICommandBus, IGameRegistry, IHostContext, IHostContextImpl, IInputService,
    ITextureService, IVfsService,
};
pub struct HostContext {
    scene_manager: ComRc<ISceneManager>,
    audio: ComRc<IAudioService>,
    textures: ComRc<ITextureService>,
    vfs: ComRc<IVfsService>,
    input: ComRc<IInputService>,
    games: ComRc<IGameRegistry>,
    commands: ComRc<ICommandBus>,
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
        commands: ComRc<ICommandBus>,
    ) -> ComRc<IHostContext> {
        ComRc::from_object(Self {
            scene_manager,
            audio,
            textures,
            vfs,
            input,
            games,
            commands,
        })
    }

    pub fn create(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        texture_factory: Rc<dyn ComponentFactory>,
        vfs: Rc<MiniFs>,
        input: Rc<RefCell<dyn InputEngine>>,
    ) -> ComRc<IHostContext> {
        let commands = CommandBus::create(None);
        Self::new(
            scene_manager,
            AudioService::create(audio_engine, vfs.clone()),
            TextureService::create(texture_factory, vfs.clone()),
            VfsService::create(vfs),
            InputService::create(input),
            GameRegistry::create(),
            commands,
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
    fn commands(&self) -> ComRc<ICommandBus> {
        self.commands.clone()
    }
}

pub use audio::{AudioService, AudioSource};
pub use command_bus::CommandBus;
pub use game_registry::GameRegistry;
pub use input::InputService;
pub use texture::{Texture, TextureService};
pub use texture_cache::ImguiTextureCache;
pub use vfs::VfsService;
