//! PAL5 story runtime (game-specific Rust, hosted in `yaobow_lib`).
//!
//! The low-level PAL5 asset/scene loaders live in `shared::openpal5`;
//! this module adds the *story runtime*: the Lua command handlers
//! ([`context`]), the Lua bridge ([`commands`]), and the per-frame
//! director ([`director`]) that runs the game scripts.

pub mod commands;
pub mod context;
pub mod director;

pub use director::Pal5StoryDirector;

use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance::scene::CoreScene;

use shared::GameType;
use shared::openpal5::asset_loader::AssetLoader;
use shared::openpal5::script::ScriptIndex;

use context::Pal5ScriptContext;

/// Build the PAL5 story director for `game` (PAL5 / PAL5Q) from
/// `asset_path`. Mirrors `Swd5Service::create_director`: it pulls the
/// engine handles off the live application, builds the per-launch vfs +
/// asset loader + script index, pushes an empty initial scene so the
/// first VM tick sees a valid scene-manager state, and returns the
/// ready `IDirector`.
///
/// Returns `None` when the scriptlist can't be read (e.g. missing PAL5
/// assets at `asset_path`) so the caller can surface a clear error.
pub fn create_story_director(
    app: ComRc<IApplication>,
    asset_path: &str,
    game: GameType,
) -> Option<ComRc<IDirector>> {
    let engine_rc = app.engine();
    let engine = engine_rc.borrow();
    let component_factory = engine.rendering_component_factory();
    let input_engine = engine.input_engine();
    let audio_engine = engine.audio_engine();
    let scene_manager = engine.scene_manager().clone();
    let ui = engine.ui_manager();
    drop(engine);

    let vfs = Rc::new(init_virtual_fs(asset_path, game.pkg_key()));
    let script_index = match ScriptIndex::load(&vfs) {
        Ok(idx) => Rc::new(idx),
        Err(e) => {
            log::error!(
                "PAL5: failed to read scriptlist.ini from {}: {}",
                asset_path,
                e
            );
            return None;
        }
    };
    let asset_loader = AssetLoader::new(component_factory.clone(), vfs);

    // Empty initial scene so the VM's first tick has a valid scene
    // manager (scripts push the real scene via BeginScene/ChangeMap).
    scene_manager.push_scene(CoreScene::create());

    let context = Pal5ScriptContext::new(
        asset_loader,
        script_index,
        scene_manager,
        component_factory,
        audio_engine,
        input_engine,
        ui,
    );

    match Pal5StoryDirector::new(context) {
        Ok(director) => Some(ComRc::<IDirector>::from_object(director)),
        Err(e) => {
            log::error!("PAL5: failed to build story director: {}", e);
            None
        }
    }
}
