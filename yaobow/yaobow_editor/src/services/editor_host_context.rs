//! Editor-flavoured host context. Wraps the shared `radiance_scripting`
//! `HostContext` services and adds the editor-only `IPreviewerHub` plus
//! the offscreen-preview registry that the editor's directors poll each
//! frame.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::audio::AudioEngine;
use radiance::comdef::ISceneManager;
use radiance::input::InputEngine;
use radiance::rendering::{ComponentFactory, RenderingEngine};
use radiance_scripting::comdef::services::{
    IAppService, IAudioService, IConfigService, IGameRegistry, IHostContext, IInputService,
    IRenderTarget, ITextureService, IVfsService,
};
use radiance_scripting::services::{
    AudioService, GameRegistry, ImguiTextureCache, InputService, ScriptedRenderTarget,
    TextureService, VfsService,
};

use crate::comdef::editor_services::{IEditorHostContext, IEditorHostContextImpl, IPreviewerHub};
use crate::comdef::services::IHostContextImpl;
use crate::directors::DevToolsAssetLoader;
use crate::services::preview_registry::PreviewRegistry;
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

    factory: Rc<dyn ComponentFactory>,
    texture_cache: Rc<RefCell<ImguiTextureCache>>,
    rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    preview_registry: Rc<PreviewRegistry>,
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
        rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    ) -> ComRc<IEditorHostContext> {
        let vfs = Rc::new(MiniFs::new(false));
        let asset_loader =
            DevToolsAssetLoader::Pal3(Rc::new(shared::openpal3::asset_manager::AssetManager::new(
                factory.clone(),
                vfs.clone(),
            )));
        let preview_registry = Rc::new(PreviewRegistry::new());
        let previewers = PreviewerHub::create(
            vfs.clone(),
            asset_loader,
            GameType::PAL3,
            factory.clone(),
            audio_engine.clone(),
            scene_manager.clone(),
            cache.clone(),
            preview_registry.clone(),
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
            cache,
            rendering_engine,
            preview_registry,
        )
    }

    /// Builds an editor host context for a specific opened game.
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
        rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    ) -> ComRc<IEditorHostContext> {
        let preview_registry = Rc::new(PreviewRegistry::new());
        let previewers = PreviewerHub::create(
            vfs.clone(),
            asset_loader,
            game_type,
            factory.clone(),
            audio_engine.clone(),
            scene_manager.clone(),
            cache.clone(),
            preview_registry.clone(),
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
            cache,
            rendering_engine,
            preview_registry,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        factory: Rc<dyn ComponentFactory>,
        vfs: Rc<MiniFs>,
        input: Rc<RefCell<dyn InputEngine>>,
        app: ComRc<IAppService>,
        config: ComRc<IConfigService>,
        previewers: ComRc<IPreviewerHub>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
        preview_registry: Rc<PreviewRegistry>,
    ) -> ComRc<IEditorHostContext> {
        ComRc::from_object(Self {
            scene_manager,
            audio: AudioService::create(audio_engine, vfs.clone()),
            textures: TextureService::create(factory.clone(), vfs.clone()),
            vfs: VfsService::create(vfs),
            input: InputService::create(input),
            games: GameRegistry::create(),
            app,
            config,
            previewers,
            factory,
            texture_cache: cache,
            rendering_engine,
            preview_registry,
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

    fn new_render_target(&self, w: i32, h: i32) -> ComRc<IRenderTarget> {
        let w = w.max(1) as u32;
        let h = h.max(1) as u32;
        let target_box = self.factory.create_render_target(w, h);
        // We discard the shared `Rc<RefCell<Box<...>>>` here because the
        // caller is the script side, which routes through the IDL
        // wrapper for all subsequent access. Scenes-to-render are
        // associated with their target inside `PreviewState`, which
        // holds its own shared handle.
        ScriptedRenderTarget::create(target_box, self.texture_cache.clone()).0
    }

    fn render_pending_previews(&self) {
        let mut engine = self.rendering_engine.borrow_mut();
        self.preview_registry.render_all(&mut *engine);
    }
}

// Allow up-casting to the shared IHostContext for components (like the
// existing welcome bootstrap) that still want the smaller interface.
pub fn as_host_context(ctx: &ComRc<IEditorHostContext>) -> Option<ComRc<IHostContext>> {
    ctx.query_interface::<IHostContext>()
}
