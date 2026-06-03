//! Host-side ComObject backing `IPal5Context`.
//!
//! Holds ONLY pal5-specific state: the asset path (per-game) and the
//! rendering component factory the `AssetLoader` needs internally.
//! Generic engine concerns (application, scene-manager, input,
//! camera) flow through the canonical `IHostContext` /
//! `IAppService` surface the protosept orchestrator
//! (`yaobow/yaobow/scripts/openpal5.p7`) already has via the `host`
//! parameter — no need to re-export them here.
//!
//! Mirrors the role `Pal4DebugContext` plays for the PAL4 debug
//! overlay: a Rust-implemented data sink the script consumes through
//! the generated `intern_pal5_context_arg` helper. The IDL interface
//! is NOT marked `[protosept(scriptable)]` — the script only calls
//! into it, never implements it.

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::IScene;
use radiance::rendering::ComponentFactory;

use crate::openpal5::asset_loader::AssetLoader;
use crate::openpal5::comdef::{IPal5Context, IPal5ContextImpl};
use crate::openpal5::scene::Pal5Scene;
use crate::GameType;

/// Default scene name baked into the legacy `OpenPal5ApplicationLoader`.
/// Kept here so the protosept orchestrator can ignore PAL5-specific
/// naming — its only knob today is "load the default scene".
const DEFAULT_SCENE_NAME: &str = "kuangfengzhai";

/// Cross-platform fallback path used by the legacy loader when
/// `YaobowConfig` had no PAL5 asset path configured.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const FALLBACK_ASSET_PATH: &str = "F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5";
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const FALLBACK_ASSET_PATH: &str = "";

pub struct Pal5Context {
    game: GameType,
    asset_path: PathBuf,
    component_factory: Rc<dyn ComponentFactory>,
}

ComObject_Pal5Context!(super::Pal5Context);

impl Pal5Context {
    /// Builds a PAL5 launch context. `asset_path` is typically
    /// `YaobowConfig::asset_path_for(GameType::PAL5)`; an empty value
    /// falls back to the legacy desktop default so existing dev
    /// workflows keep working unchanged.
    ///
    /// `game` selects between `PAL5` and `PAL5Q` so the right
    /// `.pkg` decryption key (`GameType::pkg_key()`) is threaded
    /// into the per-launch `packfs::init_virtual_fs`. Without it,
    /// every `.pkg` archive is silently skipped and packfs logs
    /// "Didn't mount … as pkg key is not provided".
    ///
    /// The component factory is needed internally by `AssetLoader`
    /// for texture / mesh decoding; it is never surfaced to the
    /// script.
    pub fn create(
        game: GameType,
        asset_path: String,
        component_factory: Rc<dyn ComponentFactory>,
    ) -> ComRc<IPal5Context> {
        let asset_path = if asset_path.is_empty() {
            PathBuf::from(FALLBACK_ASSET_PATH)
        } else {
            PathBuf::from(asset_path)
        };

        ComRc::from_object(Self {
            game,
            asset_path,
            component_factory,
        })
    }
}

impl IPal5ContextImpl for Pal5Context {
    fn load_default_scene(&self) -> Option<ComRc<IScene>> {
        let asset_path_str = self.asset_path.to_str()?;
        let vfs = init_virtual_fs(asset_path_str, self.game.pkg_key());
        let loader = AssetLoader::new(self.component_factory.clone(), Rc::new(vfs));
        match Pal5Scene::load(&loader, DEFAULT_SCENE_NAME) {
            Ok(scene) => Some(scene.scene),
            Err(err) => {
                log::warn!(
                    "Pal5Context: failed to load default scene '{}' from {}: {}",
                    DEFAULT_SCENE_NAME,
                    self.asset_path.display(),
                    err,
                );
                None
            }
        }
    }
}
