//! Editor-flavoured host context. Wraps the shared `radiance_scripting`
//! `HostContext` services and adds the editor-only `IPreviewerHub`.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::audio::AudioEngine;
use radiance::comdef::ISceneManager;
use radiance::input::InputEngine;
use radiance::rendering::ComponentFactory;
use radiance_scripting::comdef::services::{
    IAppService, IAudioService, IConfigService, IGameRegistry, IHostContext, IInputService,
    ITextureService, IVfsService,
};
use radiance_scripting::services::{
    AudioService, GameRegistry, ImguiTextureCache, InputService, TextureService, VfsService,
};

use crate::comdef::editor_services::{IEditorHostContext, IEditorHostContextImpl, IPreviewerHub};
use crate::comdef::services::IHostContextImpl;
use crate::directors::DevToolsAssetLoader;
use crate::services::previewer_hub::PreviewerHub;
use shared::GameType;

pub struct EditorHostContext {
    scene_manager: ComRc<ISceneManager>,
    audio: ComRc<IAudioService>,
    textures: ComRc<ITextureService>,
    vfs: ComRc<IVfsService>,
    input: ComRc<IInputService>,
    games: ComRc<IGameRegistry>,
    app: ComRc<IAppService>,
    config: ComRc<IConfigService>,
    previewers: ComRc<IPreviewerHub>,
}

ComObject_EditorHostContext!(super::EditorHostContext);

impl EditorHostContext {
    /// Builds an editor host context for the welcome page (no game open
    /// yet — `previewers` is wired against an empty VFS and is a no-op).
    pub fn create_welcome(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        factory: Rc<dyn ComponentFactory>,
        input: Rc<RefCell<dyn InputEngine>>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
        cache: Rc<RefCell<ImguiTextureCache>>,
    ) -> ComRc<IEditorHostContext> {
        let vfs = Rc::new(MiniFs::new(false));
        let asset_loader =
            DevToolsAssetLoader::Pal3(Rc::new(shared::openpal3::asset_manager::AssetManager::new(
                factory.clone(),
                vfs.clone(),
            )));
        let previewers = PreviewerHub::create(
            vfs.clone(),
            asset_loader,
            GameType::PAL3,
            factory.clone(),
            audio_engine.clone(),
            scene_manager.clone(),
            cache,
        );

        Self::create(
            scene_manager,
            audio_engine,
            factory,
            vfs,
            input,
            app,
            config,
            previewers,
        )
    }

    /// Builds an editor host context for a specific opened game. The
    /// previewer hub uses the game's vfs / asset loader.
    pub fn create_for_game(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        factory: Rc<dyn ComponentFactory>,
        input: Rc<RefCell<dyn InputEngine>>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        vfs: Rc<MiniFs>,
        asset_loader: DevToolsAssetLoader,
        game_type: GameType,
    ) -> ComRc<IEditorHostContext> {
        let previewers = PreviewerHub::create(
            vfs.clone(),
            asset_loader,
            game_type,
            factory.clone(),
            audio_engine.clone(),
            scene_manager.clone(),
            cache,
        );
        Self::create(
            scene_manager,
            audio_engine,
            factory,
            vfs,
            input,
            app,
            config,
            previewers,
        )
    }

    fn create(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        factory: Rc<dyn ComponentFactory>,
        vfs: Rc<MiniFs>,
        input: Rc<RefCell<dyn InputEngine>>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
        previewers: ComRc<IPreviewerHub>,
    ) -> ComRc<IEditorHostContext> {
        ComRc::from_object(Self {
            scene_manager,
            audio: AudioService::create(audio_engine, vfs.clone()),
            textures: TextureService::create(factory, vfs.clone()),
            vfs: VfsService::create(vfs),
            input: InputService::create(input),
            games: GameRegistry::create(),
            app,
            config,
            previewers,
        })
    }
}

impl IHostContextImpl for EditorHostContext {
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

impl IEditorHostContextImpl for EditorHostContext {
    fn previewers(&self) -> ComRc<IPreviewerHub> {
        self.previewers.clone()
    }
}

// Allow up-casting to the shared IHostContext for components (like the
// existing welcome bootstrap) that still want the smaller interface.
pub fn as_host_context(ctx: &ComRc<IEditorHostContext>) -> Option<ComRc<IHostContext>> {
    ctx.query_interface::<IHostContext>()
}
