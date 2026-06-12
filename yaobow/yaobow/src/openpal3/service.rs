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

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance::scene::CoreScene;
use radiance_scripting::services::ImguiTextureCache;
use shared::openpal3::asset_manager::AssetManager;
use shared::openpal3::comdef::{
    IPal3ScriptFactory, IPal3Service, IPal3ServiceImpl, IPal3StartMenuScene,
};
use shared::openpal3::directors::AdventureDirector;
use shared::openpal3::start_menu_scene::Pal3StartMenuScene;
use shared::openpal3::states::persistent_state::PAL3_APP_NAME;
use shared::scripting::sce::vm::SceExecutionOptions;
use shared::ydirs;

use crate::openpal3::debug_layer::OpenPal3DebugLayer;
use crate::openpal3::sce_proc_hooks::SceRestHooks;

/// Number of save slots surfaced by the load-menu overlay. Matches
/// the rows accepted by `AdventureDirector::load` (`PersistentState`
/// slot files `1.json`..`4.json`).
const SAVE_SLOT_COUNT: i32 = 4;

pub struct Pal3Service {
    app: ComRc<IApplication>,
    /// App-lifetime `AssetManager` cache, keyed by asset path. The
    /// start menu and the adventure director both go through the same
    /// instance so a `New Game` after browsing the menu doesn't
    /// re-mount the vfs or re-decode assets.
    asset_managers: RefCell<HashMap<String, Rc<AssetManager>>>,
    /// Imgui texture cache used by `create_start_menu` to upload the
    /// atlas pages. Installed once at boot by
    /// `YaobowApplicationLoader::on_loading`. `create_start_menu`
    /// returns `None` when this slot is empty.
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
    /// True when `create_director` has been called at least once and
    /// the debug layer install is pending. Drained by `pump_pre_update`
    /// before the next engine tick (i.e. outside any `engine.borrow()`
    /// scope) so we can call `engine().borrow_mut().set_debug_layer`
    /// without aliasing the title director's update path.
    pending_debug_install: Cell<bool>,
    debug_layer_installed: Cell<bool>,
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
            pending_debug_install: Cell::new(false),
            debug_layer_installed: Cell::new(false),
        })
    }

    /// Install the imgui texture cache so `create_start_menu` can
    /// upload the menu atlases. Called once at boot.
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
        let am = Rc::new(AssetManager::new(component_factory, Rc::new(vfs)));
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
        drop(engine);
        let debug_layer = OpenPal3DebugLayer::new(input_engine, scene_manager, ui);
        self.app
            .engine()
            .borrow_mut()
            .set_debug_layer(Box::new(debug_layer));
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

    fn sce_options() -> SceExecutionOptions {
        SceExecutionOptions {
            proc_hooks: vec![Box::new(SceRestHooks::new())],
        }
    }
}

impl IPal3ServiceImpl for Pal3Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        *self.last_asset_path.borrow_mut() = Some(asset_path.to_string());

        // Warm the AssetManager up front so the menu + adventure
        // director see a consistent VFS. The debug layer install needs
        // an exclusive engine borrow, which is not safe to take from
        // inside the title director's update path that called us; we
        // queue it for the next `pump_pre_update`.
        let _ = self.asset_manager_for(asset_path);
        self.pending_debug_install.set(true);

        // The PAL3 start menu is a pure imgui overlay — it never
        // pushes a 3D scene of its own. The engine's render loop is
        // gated on `scene_manager.scene().is_some()`, so without at
        // least one scene on the stack the imgui frame is built but
        // never blitted to the framebuffer (blank window). When we
        // come from the title director the title's scene is still
        // installed; on `--pal3` direct boot the stack starts empty,
        // so we push an empty `CoreScene` here. The adventure
        // director replaces this with its real scene when the player
        // hits New Game / Load.
        let engine_rc = self.app.engine();
        let scene_manager = engine_rc.borrow().scene_manager().clone();
        drop(engine_rc);
        if scene_manager.scene().is_none() {
            scene_manager.push_scene(CoreScene::create());
        }

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
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let input_engine = engine.input_engine();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);

        let adv = AdventureDirector::new(
            PAL3_APP_NAME,
            asset_mgr,
            audio_engine,
            input_engine,
            ui,
            scene_manager,
            Some(Self::sce_options()),
        );
        ComRc::<IDirector>::from_object(adv)
    }

    fn load_adventure_director(&self, asset_path: &str, slot: i32) -> Option<ComRc<IDirector>> {
        let asset_mgr = self.asset_manager_for(asset_path);
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let input_engine = engine.input_engine();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);

        let adv = AdventureDirector::load(
            PAL3_APP_NAME,
            asset_mgr,
            audio_engine,
            input_engine,
            ui,
            scene_manager,
            Some(Self::sce_options()),
            slot,
        )?;
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

    fn create_start_menu(&self, asset_path: &str) -> Option<ComRc<IPal3StartMenuScene>> {
        let cache = self.texture_cache.borrow().clone()?;
        let asset_mgr = self.asset_manager_for(asset_path);
        let engine_rc = self.app.engine();
        let audio_engine = engine_rc.borrow().audio_engine();
        drop(engine_rc);
        Pal3StartMenuScene::create(asset_mgr, audio_engine, cache)
    }

    fn exit_app(&self) {
        self.app.request_exit();
    }
}
