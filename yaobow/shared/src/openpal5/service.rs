//! PAL5 launch service — phase-2 replacement for `Pal5Context`.
//!
//! Exposes the two PAL5-specific Rust capabilities the script side
//! needs to build a PAL5 director:
//!
//!  * [`load_default_scene`](Pal5Service::load_default_scene) — builds
//!    the per-launch vfs, instantiates the [`AssetLoader`], and loads
//!    the default scene (`kuangfengzhai`). Returns the `IScene` ready
//!    for `scene_manager.push_scene`.
//!  * [`position_camera_default`](Pal5Service::position_camera_default)
//!    — positions the active scene's camera to the PAL5 entry pose
//!    (previously hard-coded as `INITIAL_POS_*` / `INITIAL_LOOK_*` in
//!    `openpal5.p7`).
//!
//! The PAL5 director itself stays script-implemented (the small
//! `OpenPal5Director` struct in `openpal5_director.p7`). The script
//! calls both methods + boxes the director + returns it; the engine
//! handles the install via `update()`'s return value (or via Rust
//! direct-boot calling `scene_manager.set_director` from
//! `on_loading`).

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IScene};
use radiance::rendering::ComponentFactory;

use crate::GameType;
use crate::openpal5::asset_loader::AssetLoader;
use crate::openpal5::comdef::{IPal5Service, IPal5ServiceImpl};
use crate::openpal5::scene::Pal5Scene;

/// Default scene baked into the legacy `OpenPal5ApplicationLoader`.
const DEFAULT_SCENE_NAME: &str = "kuangfengzhai";

/// PAL5 entry-pose constants (formerly the `INITIAL_POS_*` /
/// `INITIAL_LOOK_*` top-level `let` bindings in `openpal5.p7`).
const INITIAL_POS_X: f32 = 5500.0;
const INITIAL_POS_Y: f32 = 612.1155;
const INITIAL_POS_Z: f32 = 2500.0;
const INITIAL_LOOK_X: f32 = 4319.2227;
const INITIAL_LOOK_Y: f32 = 612.1155;
const INITIAL_LOOK_Z: f32 = 1708.5408;

/// Cross-platform fallback path used by the legacy loader when
/// `YaobowConfig` had no PAL5 asset path configured.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const FALLBACK_ASSET_PATH: &str = "F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5";
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const FALLBACK_ASSET_PATH: &str = "";

pub struct Pal5Service {
    app: ComRc<IApplication>,
    component_factory: Rc<dyn ComponentFactory>,
}

ComObject_Pal5Service!(super::Pal5Service);

impl Pal5Service {
    /// App-lifetime install. The `app` handle is used lazily inside
    /// [`position_camera_default`] to reach the live scene manager.
    pub fn create(
        app: ComRc<IApplication>,
        component_factory: Rc<dyn ComponentFactory>,
    ) -> ComRc<IPal5Service> {
        ComRc::from_object(Self {
            app,
            component_factory,
        })
    }
}

impl IPal5ServiceImpl for Pal5Service {
    fn load_default_scene(
        &self,
        asset_path: &str,
        game_ordinal: std::os::raw::c_int,
    ) -> Option<ComRc<IScene>> {
        let asset_path = if asset_path.is_empty() {
            PathBuf::from(FALLBACK_ASSET_PATH)
        } else {
            PathBuf::from(asset_path)
        };
        let asset_path_str = asset_path.to_str()?;
        let game =
            radiance_scripting::services::game_registry::ordinal_to_config_key(game_ordinal as i32)
                .and_then(GameType::from_config_key)
                .unwrap_or(GameType::PAL5);
        let vfs = init_virtual_fs(asset_path_str, game.pkg_key());
        let loader = AssetLoader::new(self.component_factory.clone(), Rc::new(vfs));
        match Pal5Scene::load(&loader, DEFAULT_SCENE_NAME) {
            Ok(scene) => Some(scene.scene),
            Err(err) => {
                log::warn!(
                    "Pal5Service: failed to load default scene '{}' from {}: {}",
                    DEFAULT_SCENE_NAME,
                    asset_path.display(),
                    err,
                );
                None
            }
        }
    }

    fn position_camera_default(&self) {
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let scene_manager = engine.scene_manager();
        if let Some(cam) = scene_manager.camera() {
            cam.set_position(INITIAL_POS_X, INITIAL_POS_Y, INITIAL_POS_Z);
            cam.look_at(INITIAL_LOOK_X, INITIAL_LOOK_Y, INITIAL_LOOK_Z);
        }
    }
}
