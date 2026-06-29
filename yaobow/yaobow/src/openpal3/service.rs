//! PAL3 launch service — replaces the placeholder imgui main-menu
//! with a script-built sprite-atlas start menu.
//!
//! `IPal3Service` interface lives in `shared::openpal3::comdef` (from
//! `crosscom/idl/openpal3.idl`) so it can be exposed symmetrically via
//! `IYaobowHostContext.pal3()`. The conforming `class Pal3Service` is
//! declared in `yaobow_services.idl` so the `ComObject_Pal3Service!`
//! macro generates here in `yaobow_lib` — keeping
//! `OpenPal3DebugLayer` + `sce_proc_hooks` (the surrounding
//! director-construction glue) co-located with the service.
//!
//! The PAL3 start menu itself is script-built: `create_director` QIs
//! `IPal3ScriptFactory` off the host script app and calls
//! `make_pal3_start_menu(asset_path)`. The script menu's button
//! handlers call back into this service to mint the adventure
//! director (`create_adventure_director` / `load_adventure_director`)
//! or to exit the app (`exit_app`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use agent_server::{AgentCommand, AgentError, AgentResponse};
use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::audio::Codec as AudioCodec;
use radiance::comdef::{IApplication, IApplicationExt, IDirector, ISceneManager, IUiLayer};
use radiance::input::{InputEngine, SyntheticInputBridge};
use radiance::radiance::{UiLayerBand, UiLayerHandle};
use radiance::video::Codec as VideoCodec;
use radiance_scripting::comdef::services::IAudioSource;
use radiance_scripting::comdef::services::IVideoHandle;
use radiance_scripting::comdef::services::ISpriteService;
use radiance_scripting::services::AudioSource;
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::services::SpriteService;
use shared::agent_common::AgentBridge;
use shared::loaders::video_handle::VideoHandle;
use shared::openpal3::agent::{Pal3DispatchCtx, dispatch_pal3_command};
use shared::openpal3::asset_manager::AssetManager;
use shared::openpal3::comdef::{
    IAdventureDirector, IPal3DialogRenderer, IPal3ScriptFactory, IPal3Service, IPal3ServiceImpl,
    IPal3StatusRenderer,
    IPal3UiAtlas,
};
use shared::openpal3::directors::AdventureDirector;
use shared::openpal3::ui_atlas::{AtlasManifest, Pal3UiAtlas};
use shared::openpal3::states::persistent_state::PAL3_APP_NAME;
use shared::scripting::sce::vm::SceExecutionOptions;
use shared::ydirs;
use shared::GameType;

use crate::openpal3::debug_layer::OpenPal3DebugLayer;
use crate::openpal3::sce_proc_hooks::SceRestHooks;

/// Number of save slots surfaced by the load-menu overlay. Matches
/// the rows accepted by `AdventureDirector::load` (`PersistentState`
/// slot files `1.json`..`4.json`).
const SAVE_SLOT_COUNT: i32 = 4;

/// Map a script/registry game ordinal to the PAL3-family `GameType`.
/// PAL3 and PAL3A share `Pal3Service`; anything else defaults to PAL3.
fn game_from_ordinal(ordinal: i32) -> GameType {
    match radiance_scripting::services::game_registry::ordinal_to_config_key(ordinal) {
        Some("pal3a") => GameType::PAL3A,
        _ => GameType::PAL3,
    }
}

pub struct Pal3Service {
    app: ComRc<IApplication>,
    /// App-lifetime `AssetManager` cache, keyed by asset path. The
    /// start menu and the adventure director both go through the same
    /// instance so a `New Game` after browsing the menu doesn't
    /// re-mount the vfs or re-decode assets.
    asset_managers: RefCell<HashMap<String, Rc<AssetManager>>>,
    /// Imgui texture cache shared by the generic sprite/atlas loaders
    /// (`create_sprite_service` / `create_ui_atlas`) and the dialog
    /// renderer to upload + frame-gate UI textures. Installed once at
    /// boot by `YaobowApplicationLoader::on_loading`; the loaders return
    /// `None` when this slot is empty.
    texture_cache: RefCell<Option<Rc<RefCell<ImguiTextureCache>>>>,
    /// Script-side factory (`IPal3ScriptFactory`) QI'd from the
    /// reverse-wrapped app root. Used by `create_director` to mint
    /// the scripted start menu. Cleared on app unload to break the
    /// service ↔ script-app reference cycle.
    script_factory: RefCell<Option<ComRc<IPal3ScriptFactory>>>,
    /// Set once on `create_director` so `request_exit` doesn't need
    /// to re-look-up the install path. Not strictly required; useful
    /// for diagnostics.
    last_asset_path: RefCell<Option<String>>,
    /// Game variant for the active launch, set on `create_director`.
    /// PAL3 and PAL3A share this service; this selects per-game assets
    /// such as the start-menu BGM track. Defaults to PAL3.
    last_game: Cell<GameType>,
    /// True when `create_director` has been called at least once and
    /// the debug layer install is pending. Drained by `pump_pre_update`
    /// before the next engine tick (i.e. outside any `engine.borrow()`
    /// scope) so we can register the debug-overlay UI layer on the
    /// `UiManager` without aliasing the title director's update path.
    pending_debug_install: Cell<bool>,
    debug_layer_installed: Cell<bool>,
    /// RAII handle for the registered PAL3 debug overlay UI layer. Held
    /// for the service lifetime so the layer stays registered; dropping
    /// it unregisters the overlay.
    debug_layer_handle: RefCell<Option<UiLayerHandle>>,
    /// Once-per-process latch for the LOGO.bik intro. Set to true on
    /// the first successful `play_intro_movie` call; subsequent calls
    /// return `None` so re-entry into the start menu (e.g. exit-to-menu
    /// from a future Adventure flow) skips the intro.
    intro_played: Cell<bool>,
    /// Agent-server bridge. `None` when no `--agent-port` flag was
    /// passed; `Some(_)` enables [`Self::pump_agent`] and makes
    /// every fresh `AdventureDirector` honor pause/step + see
    /// synthetic input.
    agent_bridge: RefCell<Option<Rc<AgentBridge>>>,
}

ComObject_Pal3Service!(super::Pal3Service);

impl Pal3Service {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal3Service> {
        ComRc::from_object(Self {
            app,
            asset_managers: RefCell::new(HashMap::new()),
            texture_cache: RefCell::new(None),
            script_factory: RefCell::new(None),
            last_asset_path: RefCell::new(None),
            last_game: Cell::new(GameType::PAL3),
            pending_debug_install: Cell::new(false),
            debug_layer_installed: Cell::new(false),
            debug_layer_handle: RefCell::new(None),
            intro_played: Cell::new(false),
            agent_bridge: RefCell::new(None),
        })
    }

    /// Install the imgui texture cache so the sprite/atlas loaders and
    /// dialog renderer can upload UI textures. Called once at boot.
    pub fn set_texture_cache(&self, cache: Rc<RefCell<ImguiTextureCache>>) {
        *self.texture_cache.borrow_mut() = Some(cache);
    }

    /// Install the script app's PAL3 factory.
    pub fn set_script_factory(&self, factory: ComRc<IPal3ScriptFactory>) {
        *self.script_factory.borrow_mut() = Some(factory);
    }

    /// Drop the held script factory, breaking the
    /// service ↔ host-context ↔ script-CCW reference cycle at teardown.
    pub fn clear_script_factory(&self) {
        *self.script_factory.borrow_mut() = None;
    }

    /// Build the scripted-dialog-box dependencies threaded into a fresh
    /// adventure director: a generic, game-vfs-bound `ISpriteService`
    /// (over this asset path's PAL3 vfs + the shared texture cache) and
    /// the self-contained p7 `IPal3DialogRenderer` minted by the script
    /// factory from it. The renderer owns its sprites/avatar/curtain
    /// state in p7; the host only forwards SCE events + draw calls. Both
    /// the texture cache and the script factory must already be
    /// installed (they are, by the time the player leaves the start menu).
    fn build_dialog_renderer(&self, asset_mgr: Rc<AssetManager>) -> ComRc<IPal3DialogRenderer> {
        let texture_cache = self.texture_cache.borrow().clone().expect(
            "Pal3Service::build_dialog_renderer called before the texture cache was installed. \
             YaobowApplicationLoader must call Pal3Service::set_texture_cache at boot.",
        );
        let factory = self.script_factory.borrow().clone().expect(
            "Pal3Service::build_dialog_renderer called before the script factory was installed \
             (or after it was cleared). YaobowApplicationLoader must call \
             Pal3Service::set_script_factory after installing the script root.",
        );

        // The dialog renderer's sprites still upload through the engine
        // texture cache (consumed above); the per-frame dialog draws now
        // resolve through the same cache via the engine-owned resolver on
        // `UiManager` (see `SceState::render_dialog`), so the renderer is
        // threaded straight through to the SCE VM (no wrapper struct).
        let sprites = SpriteService::create(asset_mgr.vfs_rc(), texture_cache);
        factory.make_pal3_dialog_renderer(sprites)
    }

    fn build_status_renderer(&self, asset_mgr: Rc<AssetManager>) -> ComRc<IPal3StatusRenderer> {
        let texture_cache = self.texture_cache.borrow().clone().expect(
            "Pal3Service::build_status_renderer called before the texture cache was installed. \
             YaobowApplicationLoader must call Pal3Service::set_texture_cache at boot.",
        );
        let factory = self.script_factory.borrow().clone().expect(
            "Pal3Service::build_status_renderer called before the script factory was installed \
             (or after it was cleared). YaobowApplicationLoader must call \
             Pal3Service::set_script_factory after installing the script root.",
        );
        let sprites = SpriteService::create(asset_mgr.vfs_rc(), texture_cache);
        let atlas = self
            .build_ui_atlas(asset_mgr)
            .expect("Pal3Service::build_status_renderer: failed to build UI atlas");
        factory.make_pal3_status_renderer(sprites, atlas)
    }

    /// Build the PAL3 `UI_opt.tli` atlas adapter for a given adventure
    /// asset_mgr (shared with `create_ui_atlas`, but threaded directly
    /// into the status renderer so the in-game menu screen can resolve
    /// `ui/gamemainui/*` sprites). Requires the texture cache.
    fn build_ui_atlas(&self, asset_mgr: Rc<AssetManager>) -> Option<ComRc<IPal3UiAtlas>> {
        let cache = self.texture_cache.borrow().clone()?;
        let sprites = SpriteService::create(asset_mgr.vfs_rc(), cache);
        let kind = match self.last_game.get() {
            GameType::PAL3A => AtlasManifest::Plug,
            _ => AtlasManifest::Tli,
        };
        let manifest = common::store_ext::StoreExt2::read_to_end(
            asset_mgr.vfs(),
            Pal3UiAtlas::manifest_path(kind),
        )
        .ok()?;
        Some(Pal3UiAtlas::create(sprites, &manifest, kind))
    }

    /// Returns the cached `AssetManager` for `asset_path`, mounting
    /// the vfs + building one on first use.
    fn asset_manager_for(&self, asset_path: &str) -> Rc<AssetManager> {
        if let Some(am) = self.asset_managers.borrow().get(asset_path) {
            return am.clone();
        }

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        drop(engine);

        let vfs = init_virtual_fs(&PathBuf::from(asset_path), None);
        let am = Rc::new(AssetManager::new_for_game(
            component_factory,
            Rc::new(vfs),
            self.last_game.get(),
        ));
        self.asset_managers
            .borrow_mut()
            .insert(asset_path.to_string(), am.clone());
        am
    }

    fn install_debug_layer_once(&self) {
        // Called by `pump_pre_update` *outside* any outstanding engine
        // borrow. Calling `borrow_mut()` from inside `create_director`
        // would alias the title director's update path
        // (`engine.borrow().update() -> scene_manager.update() ->
        // title_director.update() -> host.pal3().create_director`)
        // and panic at runtime. The pending-flag indirection makes the
        // install deterministic AND safe.
        if self.debug_layer_installed.get() {
            return;
        }
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let input_engine = engine.input_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        let layer: ComRc<IUiLayer> =
            ComRc::from_object(OpenPal3DebugLayer::new(input_engine, scene_manager, ui.clone()));
        // Register in the DebugOverlay band so the overlay draws on top
        // of game UI. Keep the handle alive for the service lifetime;
        // dropping it would unregister the layer.
        let handle = ui.register_ui_layer(UiLayerBand::DebugOverlay, layer);
        *self.debug_layer_handle.borrow_mut() = Some(handle);
        drop(engine);
        self.debug_layer_installed.set(true);
    }

    /// App-lifetime pre-update hook driven by
    /// `YaobowApplicationLoader::on_updating` (which runs *before*
    /// `engine.borrow().update()` each frame). Installs the PAL3 debug
    /// layer when a previous `create_director` queued it. No-op
    /// otherwise.
    pub fn pump_pre_update(&self) {
        if self.pending_debug_install.get() {
            self.pending_debug_install.set(false);
            self.install_debug_layer_once();
        }
    }

    /// Install the agent bridge so the next `create_adventure_director`
    /// / `load_adventure_director` plumbs synthetic input + pause/step
    /// gating into the new director. Called once at boot by
    /// `YaobowApplicationLoader` when `--pal3 --agent-port` is in
    /// effect.
    pub fn set_agent_bridge(&self, bridge: Rc<AgentBridge>) {
        // Forward to any director already installed so a bridge
        // attached mid-session takes effect immediately.
        if let Some(adv) = self.active_adventure_director_owned() {
            adv.inner::<AdventureDirector>()
                .set_agent_bridge(bridge.clone());
        }
        *self.agent_bridge.borrow_mut() = Some(bridge);
    }

    /// Drain the agent-server command queue, dispatch each command
    /// against PAL3 state, then publish frame telemetry and clear
    /// synthetic-input edges. Called once per frame by
    /// `YaobowApplicationLoader::on_updating` (before the engine
    /// tick), so commands land before the active director runs.
    pub fn pump_agent(&self, delta_sec: f32) {
        let bridge = match self.agent_bridge.borrow().as_ref() {
            Some(b) => b.clone(),
            None => return,
        };

        // Lazily wire the rendering engine so `/v1/screenshot` works
        // before any director has set it on the bridge.
        if bridge.rendering_engine.borrow().is_none() {
            let engine = self.app.engine().borrow().rendering_engine();
            bridge.set_rendering_engine(engine);
        }

        // Drain the queue once (single drainer for this game).
        let mut envelopes = Vec::new();
        if let Some(consumer) = bridge.consumer.borrow().as_ref() {
            consumer.drain(|env| envelopes.push(env));
        }

        let scene_manager = self.app.engine().borrow().scene_manager().clone();

        for env in envelopes {
            // Mode-control commands install the next director on the
            // scene manager; routed here because the dispatcher (not
            // any director) owns the mode graph.
            if Self::is_mode_control_command(&env.command) {
                let response = self.dispatch_mode_control(&scene_manager, env.command.clone());
                env.reply(response);
                continue;
            }

            let active = Self::active_adventure_director(&scene_manager);
            let director_ref = active.as_ref().map(|c| c.inner::<AdventureDirector>());
            let ctx = Pal3DispatchCtx {
                bridge: &bridge,
                director: director_ref.as_deref(),
                scene_manager: scene_manager.clone(),
            };
            let response = dispatch_pal3_command(&ctx, env.command.clone());
            env.reply(response);
        }

        // Telemetry: always advance frame counter + publish dt/fps.
        bridge.publish_frame_telemetry(delta_sec);

        // Clear synthetic-input edges. We do it here unconditionally
        // because PAL3's `AdventureDirector::update` runs *during*
        // `engine.update`, which is *after* this `on_updating` hook.
        // That means any tap injected this frame is observable by
        // the director's input poll, and we clear at the start of
        // the next frame's pump.
        bridge.input_bridge.borrow().end_frame();
    }

    fn is_mode_control_command(command: &AgentCommand) -> bool {
        matches!(
            command,
            AgentCommand::EnterNewGame
                | AgentCommand::EnterLoadGame(_)
                | AgentCommand::LoadSlot(_)
                | AgentCommand::ExitGame
        )
    }

    fn dispatch_mode_control(
        &self,
        scene_manager: &ComRc<ISceneManager>,
        command: AgentCommand,
    ) -> AgentResponse {
        if let AgentCommand::ExitGame = command {
            self.app.request_exit();
            return AgentResponse::Ok;
        }

        let asset_path = match self.last_asset_path.borrow().clone() {
            Some(p) => p,
            None => {
                return AgentResponse::err(AgentError::internal(
                    "PAL3 launch asset path not set yet (no playthrough has been mounted)",
                ));
            }
        };

        let director = match command {
            AgentCommand::EnterNewGame => self.create_adventure_director(&asset_path),
            AgentCommand::EnterLoadGame(params) | AgentCommand::LoadSlot(params) => {
                // PAL3 has no in-place state reload (PAL4-style); both
                // EnterLoadGame and LoadSlot rebuild the adventure
                // director from the slot file, mirroring what the menu
                // does when the player picks a save.
                match self.load_adventure_director(&asset_path, params.slot) {
                    Some(d) => d,
                    None => {
                        return AgentResponse::err(AgentError::conflict(format!(
                            "save slot {} is missing or could not be loaded",
                            params.slot
                        )));
                    }
                }
            }
            _ => {
                return AgentResponse::err(AgentError::internal(
                    "dispatch_mode_control called with a non-mode-control command",
                ));
            }
        };
        scene_manager.set_director(director);
        AgentResponse::Ok
    }

    fn active_adventure_director(
        scene_manager: &ComRc<ISceneManager>,
    ) -> Option<ComRc<IAdventureDirector>> {
        scene_manager
            .director()
            .and_then(|d| d.query_interface::<IAdventureDirector>())
    }

    fn active_adventure_director_owned(&self) -> Option<ComRc<IAdventureDirector>> {
        let sm = self.app.engine().borrow().scene_manager().clone();
        Self::active_adventure_director(&sm)
    }

    /// Resolve the `Rc<RefCell<dyn InputEngine>>` we should hand to
    /// the next `AdventureDirector`: the agent's synthetic-input
    /// bridge when present (so `/v1/input/*` reaches the director),
    /// otherwise the real engine input.
    fn input_engine_for_director(&self) -> Rc<RefCell<dyn InputEngine>> {
        if let Some(bridge) = self.agent_bridge.borrow().as_ref() {
            // Rc<RefCell<SyntheticInputBridge>> coerces to
            // Rc<RefCell<dyn InputEngine>> at the binding site.
            let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
            return synth;
        }
        self.app.engine().borrow().input_engine()
    }

    fn sce_options() -> SceExecutionOptions {
        SceExecutionOptions {
            proc_hooks: vec![Box::new(SceRestHooks::new())],
        }
    }
}

impl IPal3ServiceImpl for Pal3Service {
    fn create_director(&self, asset_path: &str, game_ordinal: i32) -> ComRc<IDirector> {
        *self.last_asset_path.borrow_mut() = Some(asset_path.to_string());
        let game = game_from_ordinal(game_ordinal);
        self.last_game.set(game);

        // Switch in-game text to the game-shipped font (simsun). No-op if
        // the file is missing; the editor/title selector keep the bundled
        // font. Registered once here; the engine rebuilds the atlas next
        // frame.
        if let Some(bytes) = shared::load_game_font(shared::GameType::PAL3, asset_path) {
            self.app
                .engine()
                .borrow()
                .ui_manager()
                .add_game_font(&bytes, shared::GameType::PAL3.ui_font_scale());
        }

        // Warm the AssetManager up front so the menu + adventure
        // director see a consistent VFS. The debug layer install needs
        // an exclusive engine borrow, which is not safe to take from
        // inside the title director's update path that called us; we
        // queue it for the next `pump_pre_update`.
        let _ = self.asset_manager_for(asset_path);
        self.pending_debug_install.set(true);

        let factory = self.script_factory.borrow().clone().expect(
            "Pal3Service::create_director called before the script factory was installed \
             (or after it was cleared). YaobowApplicationLoader must call \
             Pal3Service::set_script_factory after installing the script root.",
        );

        let menu = factory.make_pal3_start_menu(asset_path);
        match menu.query_interface::<IDirector>() {
            Some(director) => director,
            None => {
                log::warn!(
                    "Pal3Service::create_director: scripted start menu did not expose IDirector \
                     for {}; falling back to a fresh adventure director",
                    asset_path
                );
                self.create_adventure_director(asset_path)
            }
        }
    }

    fn create_adventure_director(&self, asset_path: &str) -> ComRc<IDirector> {
        let asset_mgr = self.asset_manager_for(asset_path);
        let dialog_renderer = self.build_dialog_renderer(asset_mgr.clone());
        let status_renderer = self.build_status_renderer(asset_mgr.clone());
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);
        let input_engine = self.input_engine_for_director();

        let adv = AdventureDirector::new(
            PAL3_APP_NAME,
            asset_mgr,
            audio_engine,
            input_engine,
            ui,
            scene_manager,
            Some(Self::sce_options()),
            dialog_renderer,
            status_renderer,
        );
        if let Some(bridge) = self.agent_bridge.borrow().as_ref() {
            adv.set_agent_bridge(bridge.clone());
        }
        ComRc::<IDirector>::from_object(adv)
    }

    fn load_adventure_director(&self, asset_path: &str, slot: i32) -> Option<ComRc<IDirector>> {
        let asset_mgr = self.asset_manager_for(asset_path);
        let dialog_renderer = self.build_dialog_renderer(asset_mgr.clone());
        let status_renderer = self.build_status_renderer(asset_mgr.clone());
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);
        let input_engine = self.input_engine_for_director();

        let adv = AdventureDirector::load(
            PAL3_APP_NAME,
            asset_mgr,
            audio_engine,
            input_engine,
            ui,
            scene_manager,
            Some(Self::sce_options()),
            slot,
            dialog_renderer,
            status_renderer,
        )?;
        if let Some(bridge) = self.agent_bridge.borrow().as_ref() {
            adv.set_agent_bridge(bridge.clone());
        }
        Some(ComRc::<IDirector>::from_object(adv))
    }

    fn save_slot_exists(&self, slot: i32) -> bool {
        if slot < 1 || slot > SAVE_SLOT_COUNT {
            return false;
        }
        ydirs::save_dir()
            .join(PAL3_APP_NAME)
            .join("Save")
            .join(format!("{}.json", slot))
            .is_file()
    }

    fn save_slot_count(&self) -> i32 {
        SAVE_SLOT_COUNT
    }

    fn load_menu_bgm(&self, asset_path: &str) -> Option<ComRc<IAudioSource>> {
        let asset_mgr = self.asset_manager_for(asset_path);
        let engine_rc = self.app.engine();
        let audio_engine = engine_rc.borrow().audio_engine();
        drop(engine_rc);
        // PAL3A's menu BGM track differs from PAL3's. Both share this
        // service; pick by the game variant captured in create_director.
        let track = match self.last_game.get() {
            GameType::PAL3A => "P01",
            _ => "PI01",
        };
        // `load_music_data` panics on a missing track; isolate it so a
        // partial asset set still yields a (silent) menu.
        let data = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            asset_mgr.load_music_data(track)
        }))
        .ok()?;
        let mut source = audio_engine.create_source();
        source.set_data(data, AudioCodec::Mp3);
        Some(AudioSource::create(source))
    }

    fn create_sprite_service(&self, asset_path: &str) -> Option<ComRc<ISpriteService>> {
        let cache = self.texture_cache.borrow().clone()?;
        let asset_mgr = self.asset_manager_for(asset_path);
        Some(SpriteService::create(asset_mgr.vfs_rc(), cache))
    }

    fn create_ui_atlas(&self, asset_path: &str) -> Option<ComRc<IPal3UiAtlas>> {
        let sprites = self.create_sprite_service(asset_path)?;
        let asset_mgr = self.asset_manager_for(asset_path);
        // PAL3A replaces PAL3's text UI_opt.tli with the UIArtist.plug
        // manifest. Pick the format by the active game variant.
        let kind = match self.last_game.get() {
            GameType::PAL3A => AtlasManifest::Plug,
            _ => AtlasManifest::Tli,
        };
        let manifest = common::store_ext::StoreExt2::read_to_end(
            asset_mgr.vfs(),
            Pal3UiAtlas::manifest_path(kind),
        )
        .ok()?;
        Some(Pal3UiAtlas::create(sprites, &manifest, kind))
    }

    fn exit_app(&self) {
        self.app.request_exit();
    }

    fn play_intro_movie(&self, asset_path: &str) -> Option<ComRc<IVideoHandle>> {
        // PAL3A ships no intro movie; skip straight to the menu.
        if self.last_game.get() == GameType::PAL3A {
            return None;
        }
        // Once-per-process: re-entry into the menu must skip the intro.
        if self.intro_played.get() {
            return None;
        }
        let cache = self.texture_cache.borrow().clone()?;
        let asset_mgr = self.asset_manager_for(asset_path);
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        let audio_engine = engine.audio_engine();
        drop(engine);

        let reader = asset_mgr.load_movie_data("LOGO");
        let mut player = component_factory.create_video_player();
        let size = player.play(
            component_factory.clone(),
            audio_engine,
            reader,
            VideoCodec::Bik,
            false,
        )?;
        self.intro_played.set(true);
        Some(VideoHandle::create(cache, player, size.0, size.1))
    }
}
