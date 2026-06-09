//! PAL4 launch service — phase-2 replacement for `Pal4LaunchContext`.
//!
//! App-lifetime singleton installed by `YaobowApplicationLoader` and
//! exposed via `IYaobowHostContext.pal4()`. `create_director(path)`
//! returns a fully-wired `IDirector` (= `OpenPAL4Director`) with:
//!  * its AssetLoader bound to the per-launch vfs
//!  * the agent bridge attached (if the loader pre-stocked one via
//!    [`Pal4Service::set_agent_bridge`])
//!  * the debug-overlay bundle attached (script-built via the
//!    `YaobowScriptProject`'s `make_pal4_debug_bundle`)
//!  * the actor controller factory attached (script-built)
//!
//! ## Ownership graph
//!
//! `Pal4Service` is held by `YaobowHostContext` which is interned into
//! the script via `foreign_box`. The script root receives the host
//! context as `box<IYaobowHostContext>`. `YaobowScriptProject` is held
//! as an engine service. To avoid a Rc cycle
//! (`YaobowScriptProject` → `ScriptHost` → `ComObjectTable` →
//! `YaobowHostContext` → `Pal4Service` → `YaobowScriptProject`), the
//! reference back to the project here is a [`Weak`].
//!
//! ## Agent server lifetime
//!
//! `Pal4Service` only holds the [`Pal4AgentBridge`] handle. The
//! companion `AgentServer` HTTP listener lives on
//! `YaobowApplicationLoader` (Rust-side, RefCell<Option<AgentServer>>)
//! for the app lifetime — joined deterministically at process exit.
//! This separation was flagged as load-bearing by phase-1's
//! rubber-duck (finding A).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector, IScene};
use radiance::input::{InputEngine, SyntheticInputBridge};
use radiance::audio::Codec;
use radiance_scripting::comdef::services::{IAudioSource, IUiLayoutHandle};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::services::audio::AudioSource as ScriptAudioSource;

use crate::loaders::cegui::layout as cegui_layout;
use crate::loaders::cegui::ui_layout_handle::UiLayoutHandle;
use crate::openpal4::agent::Pal4AgentBridge;
use crate::openpal4::asset_loader::AssetLoader;
use crate::openpal4::comdef::{IPal4Service, IPal4ServiceImpl};
use crate::openpal4::director::OpenPAL4Director;
use crate::openpal4::scene::Pal4ActorControllerFactory;
use crate::openpal4::states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState};
use common::store_ext::StoreExt2;

/// PAL4 save namespace lives in
/// `openpal4::states::persistent_state::PAL4_APP_NAME` so the service
/// and `Pal4AppContext` agree on the slot directory.


/// Trait the script project exposes for `Pal4Service` to mint the
/// PAL4 debug overlay bundle and the start-menu director during
/// launch. Decouples `shared::openpal4::service` from
/// `yaobow_lib::script_source` so `shared` doesn't need to know about
/// `YaobowScriptProject`.
///
/// `yaobow_lib::script_source::YaobowScriptProject` implements this
/// trait; the service holds it as `Weak<dyn Pal4ScriptHooks>` to
/// break the project↔host-context↔service Rc cycle.
///
/// The actor-controller factory is NOT part of this trait — it
/// needs `Rc<YaobowScriptProject>` to clone the project into each
/// minted controller, which doesn't fit the `&self` trait shape.
/// Instead, `YaobowApplicationLoader` calls
/// [`Pal4Service::set_actor_controller_factory`] explicitly after
/// the script project is installed.
pub trait Pal4ScriptHooks {
    fn make_pal4_debug_bundle(&self) -> crate::openpal4::director::Pal4DebugBundle;

    /// Build the script-side PAL4 start-menu director against the
    /// per-launch `asset_path` (already stocked into the service's
    /// vfs slot — the script reaches the vfs via
    /// `host.pal4().open_layout(...)`). Returns `None` when the
    /// menu's layout / imagesets can't be loaded (e.g. PAL4 not
    /// installed at this path).
    fn make_pal4_start_menu(&self, asset_path: &str) -> Option<ComRc<IDirector>>;
}

pub struct Pal4Service {
    app: ComRc<IApplication>,
    agent_bridge: RefCell<Option<Rc<Pal4AgentBridge>>>,
    /// Weak ref to the yaobow script project (for minting the debug
    /// overlay). Set after construction via
    /// [`Pal4Service::set_script_hooks`] because the project itself
    /// depends on the host context (which holds this service)
    /// existing first. Wrapped in `Option` because `Weak::new()`
    /// cannot mint a `Weak<dyn Trait>` without a concrete starting
    /// value on stable Rust.
    script_hooks: RefCell<Option<Weak<dyn Pal4ScriptHooks>>>,
    /// Scripted actor-controller factory. Set explicitly by
    /// `YaobowApplicationLoader` after the script project is
    /// installed. Cloned into each constructed director.
    actor_controller_factory: RefCell<Option<Rc<dyn Pal4ActorControllerFactory>>>,

    /// App-lifetime `AssetLoader` for PAL4. Built lazily by
    /// `loader_for` on the first `create_director` (mounting the vfs and
    /// picking the real vs. agent-synthetic input engine), then reused
    /// for the whole process: the single asset front-door for menu-time
    /// loads (`open_layout`, `load_menu_scene`, `load_music`) and handed
    /// straight to the story director by `build_story_director`. The
    /// PAL4 asset path is invariant for a launch (the menu captures it
    /// once and passes the same string to every story-director factory),
    /// and `AssetLoader` is immutable after construction, so a single
    /// instance is mounted once and never remounted.
    launch_loader: RefCell<Option<Rc<AssetLoader>>>,

    /// Imgui texture cache used by `open_layout` to upload imageset
    /// atlases. Set once at boot by `YaobowApplicationLoader` after
    /// `install_imgui_pump` returns the cache. `open_layout` returns
    /// `None` when this slot is empty (e.g. on a build target with
    /// no imgui pump).
    texture_cache: RefCell<Option<Rc<RefCell<ImguiTextureCache>>>>,

    /// Reusable scratch buffer backing `save_slot_summary`'s `&str`
    /// return. The accessor refreshes it then returns a pointer into
    /// it; codegen copies into a CString immediately.
    summary_scratch: RefCell<String>,
}

ComObject_Pal4Service!(super::Pal4Service);

impl Pal4Service {
    /// App-lifetime install. All slots are late-bound via setters
    /// because their construction depends on the host context (which
    /// holds this service) existing first, or on the imgui pump
    /// being installed.
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal4Service> {
        ComRc::from_object(Self {
            app,
            agent_bridge: RefCell::new(None),
            script_hooks: RefCell::new(None),
            actor_controller_factory: RefCell::new(None),
            launch_loader: RefCell::new(None),
            texture_cache: RefCell::new(None),
            summary_scratch: RefCell::new(String::new()),
        })
    }

    /// Install the imgui texture cache so `open_layout` can upload
    /// imageset atlases. Called once at boot by
    /// `YaobowApplicationLoader::on_loading` with the cache returned
    /// by `install_imgui_pump`. Idempotent — replaces a previous
    /// cache (in tests / hot-reload scenarios).
    pub fn set_texture_cache(&self, cache: Rc<RefCell<ImguiTextureCache>>) {
        *self.texture_cache.borrow_mut() = Some(cache);
    }

    /// Install the agent bridge. Called by `YaobowApplicationLoader`
    /// during `on_loading` when the binary was started with
    /// `--pal4 --agent-port`. Idempotent — re-installation replaces
    /// the previous bridge; a warn is logged when this happens since
    /// double-install would indicate a sequencing bug.
    pub fn set_agent_bridge(&self, bridge: Rc<Pal4AgentBridge>) {
        let mut slot = self.agent_bridge.borrow_mut();
        if slot.is_some() {
            log::warn!("Pal4Service::set_agent_bridge called twice; replacing previous bridge");
        }
        *slot = Some(bridge);
    }

    /// Install the script-project hook. Called by
    /// `YaobowApplicationLoader::on_loading` after
    /// `YaobowScriptProject::install` returns.
    pub fn set_script_hooks(&self, hooks: Weak<dyn Pal4ScriptHooks>) {
        *self.script_hooks.borrow_mut() = Some(hooks);
    }

    /// Install the scripted actor-controller factory. Called by
    /// `YaobowApplicationLoader::on_loading` after the script project
    /// is installed.
    pub fn set_actor_controller_factory(&self, factory: Rc<dyn Pal4ActorControllerFactory>) {
        *self.actor_controller_factory.borrow_mut() = Some(factory);
    }
    /// Returns the app-lifetime PAL4 `AssetLoader`, mounting + caching
    /// it on first use. Every subsequent call returns the same cached
    /// `Rc<AssetLoader>` regardless of `asset_path` (which is invariant
    /// across a PAL4 launch).
    ///
    /// This is the single asset front-door for a PAL4 launch: the
    /// start menu reads through it (`open_layout`, `load_menu_scene`,
    /// `load_music`) and the story director receives the very same
    /// `Rc<AssetLoader>` — no second mount, no ownership handoff.
    ///
    /// The loader's input engine is the agent's `SyntheticInputBridge`
    /// when an agent bridge is installed (so `/v1/input/*` commands are
    /// observable by every consumer), otherwise the real engine input.
    /// The agent bridge is installed at boot, before any
    /// `create_director`, so this choice is stable across a launch.
    fn loader_for(&self, asset_path: &str) -> Rc<AssetLoader> {
        if let Some(loader) = self.launch_loader.borrow().as_ref() {
            return loader.clone();
        }

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        let real_input = engine.input_engine();
        drop(engine);

        let input_engine: Rc<RefCell<dyn InputEngine>> = match self.agent_bridge.borrow().as_ref() {
            Some(bridge) => bridge.input_bridge.clone(),
            None => real_input,
        };

        let vfs = init_virtual_fs(asset_path, None);
        let loader = AssetLoader::new(component_factory, input_engine, vfs);
        *self.launch_loader.borrow_mut() = Some(loader.clone());
        loader
    }
}

impl IPal4ServiceImpl for Pal4Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        // Mount the per-launch asset loader now so the script-side
        // menu can call `host.pal4().open_layout("/gamedata/ui/...")`.
        let _ = self.loader_for(asset_path);

        let hooks = self
            .script_hooks
            .borrow()
            .as_ref()
            .and_then(|w| w.upgrade())
            .expect(
                "Pal4Service::create_director called before YaobowScriptProject was installed \
                 (or after it was dropped). The YaobowApplicationLoader must call \
                 Pal4Service::set_script_hooks after installing the script project.",
            );

        match hooks.make_pal4_start_menu(asset_path) {
            Some(menu) => menu,
            None => {
                log::warn!(
                    "Pal4Service::create_director: scripted start menu failed to build for {}; \
                     falling back to story director",
                    asset_path
                );
                self.create_story_director(asset_path)
            }
        }
    }

    fn create_story_director(&self, asset_path: &str) -> ComRc<IDirector> {
        ComRc::<IDirector>::from_object(self.build_story_director(asset_path))
    }

    fn create_story_director_from_save(&self, asset_path: &str, slot: i32) -> ComRc<IDirector> {
        let director = self.build_story_director(asset_path);
        director.set_pending_load_slot(slot);
        ComRc::<IDirector>::from_object(director)
    }

    fn save_slot_count(&self) -> i32 {
        Pal4PersistentState::SLOT_COUNT
    }

    fn save_slot_exists(&self, slot: i32) -> bool {
        Pal4PersistentState::peek(PAL4_APP_NAME, slot).is_some()
    }

    fn save_slot_summary(&self, slot: i32) -> &str {
        // Full, display-ready row label for the load screen's slot
        // list. ASCII only — the PAL4 menu imgui font has no CJK
        // glyphs. Populated slots show their scene/quest summary;
        // empty slots are explicitly marked so the row still renders.
        let label = match Pal4PersistentState::peek(PAL4_APP_NAME, slot) {
            Some(state) => format!("Slot {} - {}", slot, state.summary()),
            None => format!("Slot {} - (empty)", slot),
        };
        *self.summary_scratch.borrow_mut() = label;
        unsafe { (*self.summary_scratch.as_ptr()).as_str() }
    }
    fn open_layout(&self, vfs_path: &str) -> Option<ComRc<IUiLayoutHandle>> {
        let cache = self.texture_cache.borrow().clone()?;
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        let vfs = loader.vfs();
        // Sanity-check: the file should look like a CEGUI layout
        // before we incur the (cheap) parse + atlas upload cost.
        if let Ok(bytes) = vfs.read_to_end(vfs_path) {
            if !cegui_layout::looks_like_gui_layout(&bytes) {
                log::warn!(
                    "Pal4Service::open_layout({vfs_path}): file does not look like <GUILayout>"
                );
                return None;
            }
        }
        UiLayoutHandle::try_create(vfs, vfs_path, cache)
    }

    fn load_menu_scene(&self) -> Option<ComRc<IScene>> {
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        loader.load_menu_scene()
    }

    fn load_music(&self, music_name: &str) -> Option<ComRc<IAudioSource>> {
        // Delegate the `/gamedata/Music/<name>.smp` read + XXTEA
        // decrypt to the single asset front-door so the path / decode
        // lives once (the in-game BGM path uses the same
        // `AssetLoader::load_music`).
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        let decrypted = match loader.load_music(music_name) {
            Ok(d) => d,
            Err(err) => {
                log::warn!("Pal4Service::load_music({music_name}): {err:#}");
                return None;
            }
        };

        let engine_rc = self.app.engine();
        let audio_engine = engine_rc.borrow().audio_engine();
        drop(engine_rc);

        let mut source = audio_engine.create_source();
        source.set_data(decrypted, Codec::Mp3);
        Some(ScriptAudioSource::create(source))
    }
}

impl Pal4Service {
    /// Construct (but do not wrap) the full PAL4 story director: asset
    /// loader, AngelScript VM, agent bridge, debug bundle, and actor
    /// controller factory. Shared by `create_story_director` and
    /// `create_story_director_from_save`; the latter sets a pending
    /// load slot on the returned director before wrapping it.
    fn build_story_director(&self, asset_path: &str) -> OpenPAL4Director {
        let hooks = self
            .script_hooks
            .borrow()
            .as_ref()
            .and_then(|w| w.upgrade())
            .expect(
                "Pal4Service::create_story_director called before YaobowScriptProject was installed",
            );

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();

        let component_factory = engine.rendering_component_factory();
        let real_input = engine.input_engine();
        let task_manager = engine.task_manager();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        let rendering_engine = engine.rendering_engine();
        drop(engine);

        let agent_bridge = self.agent_bridge.borrow().clone();

        // Agent mode wraps the engine input so commands posted via
        // `/v1/input/*` are observable by every consumer (scripts,
        // actor controllers, the director's own polls). Without an
        // agent, the real input handle is used unchanged. This mirrors
        // the choice `loader_for` makes for the AssetLoader below.
        let input_engine: Rc<RefCell<dyn InputEngine>> = match &agent_bridge {
            Some(bridge) => {
                let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
                synth
            }
            None => real_input,
        };

        // Reuse the single app-lifetime AssetLoader the start menu (or
        // our own `create_director`) already mounted — the director and
        // the menu share one asset front-door instead of remounting a
        // second `MiniFs`.
        let loader = self.loader_for(asset_path);

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager,
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );

        director.set_debug_bundle(hooks.make_pal4_debug_bundle());
        if let Some(factory) = self.actor_controller_factory.borrow().clone() {
            director.set_actor_controller_factory(factory);
        }

        if let Some(bridge) = agent_bridge {
            bridge.set_rendering_engine(rendering_engine);
            director.set_agent_bridge(bridge);
        }

        director
    }
}
