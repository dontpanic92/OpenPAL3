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
use mini_fs::MiniFs;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector, IScene};
use radiance::input::{InputEngine, SyntheticInputBridge};
use radiance::scene::CoreScene;
use radiance::audio::Codec;
use radiance_scripting::comdef::services::{IAudioSource, IUiLayoutHandle};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::services::audio::AudioSource as ScriptAudioSource;

use crate::loaders::cegui::layout as cegui_layout;
use crate::loaders::cegui::ui_layout_handle::UiLayoutHandle;
use crate::loaders::dff::{DffLoaderConfig, create_entity_from_dff_model};
use crate::loaders::bsp::create_entity_from_bsp_model;
use crate::loaders::Pal4TextureResolver;
use crate::openpal4::agent::Pal4AgentBridge;
use crate::openpal4::asset_loader::AssetLoader;
use crate::openpal4::comdef::{IPal4Service, IPal4ServiceImpl};
use crate::openpal4::director::OpenPAL4Director;
use crate::openpal4::scene::Pal4ActorControllerFactory;
use crate::openpal4::uv_anim::UvAnimDriver;
use common::store_ext::StoreExt2;
use fileformats::rwbs::uva::UvAnimDict;

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

    /// Per-launch vfs for the **current** PAL4 launch. Populated by
    /// `create_director` (and refreshed by `create_story_director`
    /// if its `asset_path` differs), then consumed by `open_layout`
    /// for the start menu and by `create_story_director` for the
    /// AssetLoader. Cleared on a fresh `create_director` with a
    /// different path.
    launch_vfs: RefCell<Option<(String, Rc<MiniFs>)>>,

    /// Imgui texture cache used by `open_layout` to upload imageset
    /// atlases. Set once at boot by `YaobowApplicationLoader` after
    /// `install_imgui_pump` returns the cache. `open_layout` returns
    /// `None` when this slot is empty (e.g. on a build target with
    /// no imgui pump).
    texture_cache: RefCell<Option<Rc<RefCell<ImguiTextureCache>>>>,

    /// `UvAnimDriver` for the start-menu's water + trans overlays.
    /// Populated by `load_menu_scene`, ticked by `tick_menu` each
    /// frame, cleared by `unload_menu_scene`. `None` outside the
    /// menu's active window.
    menu_uv_anim: RefCell<Option<UvAnimDriver>>,
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
            launch_vfs: RefCell::new(None),
            texture_cache: RefCell::new(None),
            menu_uv_anim: RefCell::new(None),
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
    /// Returns the per-launch vfs for `asset_path`, mounting + caching
    /// it on first use. Subsequent calls with the same path return
    /// the cached `Rc<MiniFs>`; a different path remounts.
    fn vfs_for(&self, asset_path: &str) -> Rc<MiniFs> {
        let mut slot = self.launch_vfs.borrow_mut();
        if let Some((path, vfs)) = slot.as_ref() {
            if path == asset_path {
                return vfs.clone();
            }
        }
        let vfs = Rc::new(init_virtual_fs(asset_path, None));
        *slot = Some((asset_path.to_string(), vfs.clone()));
        vfs
    }
}

impl IPal4ServiceImpl for Pal4Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        // Mount the per-launch vfs now so the script-side menu can
        // call `host.pal4().open_layout("/gamedata/ui/layouts/...")`.
        let _ = self.vfs_for(asset_path);

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
        // agent, the real input handle is used unchanged.
        let input_engine: Rc<RefCell<dyn InputEngine>> = match &agent_bridge {
            Some(bridge) => {
                let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
                synth
            }
            None => real_input,
        };

        // Reuse the launch vfs the start menu (or our own
        // `create_director`) already mounted. `Rc::try_unwrap` would
        // be hostile to the menu still holding a reference, so we
        // clone the Rc and then deep-clone the underlying MiniFs
        // for the asset loader.
        let vfs_rc = self.vfs_for(asset_path);
        let vfs = Rc::try_unwrap(vfs_rc).unwrap_or_else(|rc| {
            // AssetLoader needs an owned MiniFs; if the menu is
            // still holding a reference (e.g. transitional frame),
            // remount cheaply so neither side observes a corrupt fs.
            log::debug!(
                "Pal4Service::create_story_director: launch vfs still shared; remounting fresh \
                 MiniFs from {asset_path}"
            );
            let _ = rc; // keep the existing share live
            init_virtual_fs(asset_path, None)
        });
        let loader = AssetLoader::new(component_factory.clone(), input_engine.clone(), vfs);

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

        // Story director is now sole owner of the launch vfs; drop
        // the cached slot so a later restart cleanly remounts.
        self.launch_vfs.borrow_mut().take();

        ComRc::<IDirector>::from_object(director)
    }

    fn open_layout(&self, vfs_path: &str) -> Option<ComRc<IUiLayoutHandle>> {
        let cache = self.texture_cache.borrow().clone()?;
        let vfs_slot = self.launch_vfs.borrow();
        let (_, vfs) = vfs_slot.as_ref()?;
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
        let vfs_slot = self.launch_vfs.borrow();
        let (_, vfs) = vfs_slot.as_ref()?;
        let engine_rc = self.app.engine();
        let component_factory = engine_rc.borrow().rendering_component_factory();
        drop(engine_rc);

        match load_zjm_world(&component_factory, vfs) {
            Ok((scene, driver)) => {
                *self.menu_uv_anim.borrow_mut() = Some(driver);
                Some(scene)
            }
            Err(err) => {
                log::warn!("Pal4Service::load_menu_scene: {:#}", err);
                None
            }
        }
    }

    fn tick_menu(&self, delta_sec: f32) {
        if let Some(driver) = self.menu_uv_anim.borrow_mut().as_mut() {
            driver.tick(delta_sec);
        }
    }

    fn unload_menu_scene(&self) {
        self.menu_uv_anim.borrow_mut().take();
    }

    fn load_music(&self, music_name: &str) -> Option<ComRc<IAudioSource>> {
        let vfs_slot = self.launch_vfs.borrow();
        let (_, vfs) = vfs_slot.as_ref()?;
        let path = format!("/gamedata/Music/{}.smp", music_name);
        let raw = match vfs.read_to_end(&path) {
            Ok(b) => b,
            Err(err) => {
                log::warn!("Pal4Service::load_music({music_name}): {path} not found: {err}");
                return None;
            }
        };
        let decrypted = match crate::loaders::smp::load_smp(raw) {
            Ok(d) => d,
            Err(err) => {
                log::warn!(
                    "Pal4Service::load_music({music_name}): smp decrypt failed: {err:#}"
                );
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

// ---------------------------------------------------------------------------
// ZJM start-menu world loader
// ---------------------------------------------------------------------------

// Vfs paths inside the per-launch PAL4 vfs (ui.cpk mounts at
// `/gamedata/ui/`).
const ZJM_BSP_PATH: &str = "/gamedata/ui/uiWorld/zjm/ZJM.bsp";
const ZJM_WATER_DFF: &str = "/gamedata/ui/uiWorld/zjm/ZJM_water.dff";
const ZJM_WATER_UVA: &str = "/gamedata/ui/uiWorld/zjm/ZJM_water.uva";
const ZJM_TRANS_DFF: &str = "/gamedata/ui/uiWorld/zjm/ZJM_trans.dff";
const ZJM_TRANS_UVA: &str = "/gamedata/ui/uiWorld/zjm/ZJM_trans.uva";

/// Build the ZJM start-menu scene: BSP + water + translucent overlay,
/// plus a `UvAnimDriver` carrying the paired water/trans `.uva`
/// animations. Returns `Err` only when the BSP itself can't be loaded;
/// missing water / trans / uva files are treated as optional and only
/// logged at debug.
fn load_zjm_world(
    component_factory: &Rc<dyn radiance::rendering::ComponentFactory>,
    vfs: &MiniFs,
) -> anyhow::Result<(ComRc<IScene>, UvAnimDriver)> {
    let texture_resolver = Pal4TextureResolver;

    // BSP: identity lightmap tint — UI worlds don't ship a
    // `_ltMap.cfg`. `bsp_lightmap_tint = None` falls back to the
    // identity-modulation path inside the BSP material builder.
    let bsp_config = DffLoaderConfig {
        texture_resolver: &texture_resolver,
        keep_right_to_render_only: false,
        force_unique_materials: false,
        ignore_root_frame_translation: false,
        bsp_lightmap_tint: None,
    };
    let bsp = create_entity_from_bsp_model(
        component_factory,
        vfs,
        ZJM_BSP_PATH,
        "zjm_world".to_string(),
        &bsp_config,
    )?;

    let scene = CoreScene::create();
    scene.add_entity(bsp);

    let mut driver = UvAnimDriver::new();

    // Water DFF + UVA. `force_unique_materials = true` so per-frame
    // UV xform mutations don't bleed into unrelated geometry.
    let overlay_config = DffLoaderConfig {
        texture_resolver: &texture_resolver,
        keep_right_to_render_only: false,
        force_unique_materials: true,
        ignore_root_frame_translation: false,
        bsp_lightmap_tint: None,
    };
    attach_uv_overlay(
        &scene,
        &mut driver,
        component_factory,
        vfs,
        ZJM_WATER_DFF,
        ZJM_WATER_UVA,
        "zjm_water",
        &overlay_config,
    );
    attach_uv_overlay(
        &scene,
        &mut driver,
        component_factory,
        vfs,
        ZJM_TRANS_DFF,
        ZJM_TRANS_UVA,
        "zjm_trans",
        &overlay_config,
    );

    Ok((scene, driver))
}

/// Helper: optionally load `dff_path` as a UV-animated overlay and
/// register its sibling `uva_path` on `driver`. Missing files are
/// logged at debug and skipped — the start menu degrades gracefully
/// to BSP-only if a Steam install ever ships without these assets.
fn attach_uv_overlay(
    scene: &ComRc<IScene>,
    driver: &mut UvAnimDriver,
    component_factory: &Rc<dyn radiance::rendering::ComponentFactory>,
    vfs: &MiniFs,
    dff_path: &str,
    uva_path: &str,
    name: &str,
    config: &DffLoaderConfig<'_>,
) {
    let entity = match create_entity_from_dff_model(
        component_factory,
        vfs,
        dff_path,
        name.to_string(),
        true,
        config,
    ) {
        Ok(e) => e,
        Err(err) => {
            log::debug!("Pal4Service: optional menu overlay {dff_path} missing: {err:#}");
            return;
        }
    };
    scene.add_entity(entity.clone());

    let Ok(uva_bytes) = vfs.read_to_end(uva_path) else {
        log::debug!("Pal4Service: menu overlay {dff_path} has no sibling {uva_path}");
        return;
    };
    match UvAnimDict::read_from_bytes(&uva_bytes) {
        Ok(dict) => {
            log::info!(
                "Pal4Service: {name} loaded with {} UV animation(s) from {uva_path}",
                dict.animations.len()
            );
            driver.register_water_entity(entity, &dict);
        }
        Err(err) => {
            log::warn!("Pal4Service: failed to parse {uva_path}: {err}");
        }
    }
}
