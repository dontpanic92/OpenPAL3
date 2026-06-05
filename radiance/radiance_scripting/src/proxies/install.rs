//! One-liner helper to install the imgui immediate-mode pump on the
//! engine. Every bootstrap (title, PAL4 loader, editor welcome,
//! editor `open_game`) previously hand-wired the same four pieces:
//!
//!   1. `ImguiTextureCache::new(factory)`
//!   2. `ImguiUiHost::create()`
//!   3. `ImguiImmediateDirectorPump::new(ui_manager, textures, ui_host)`
//!   4. `engine.clear_immediate_director_pump(); engine.set_immediate_director_pump(pump)`
//!
//! [`install_imgui_pump`] does all four. The texture cache it
//! constructs is returned so callers that need to share it across
//! later director swaps (the editor's previewer hub) can pass it
//! around. Callers that already own a texture cache (because they
//! want to preserve previewer registrations across an `open_game`
//! transition) use [`install_imgui_pump_with_cache`].

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::IApplication;
use radiance::comdef::IApplicationExt;

use crate::ImguiImmediateDirectorPump;
use crate::script_bridges::radiance::register_immediate_director_proto;
use crate::services::ImguiTextureCache;
use crate::services::ui_host::ImguiUiHost;
use radiance::comdef::IUiHost;

/// Install a fresh imgui pump on the engine and return the texture
/// cache it owns. Any previously installed pump is cleared first.
pub fn install_imgui_pump(app: &ComRc<IApplication>) -> Rc<RefCell<ImguiTextureCache>> {
    let factory = app.engine().borrow().rendering_component_factory();
    let textures = Rc::new(RefCell::new(ImguiTextureCache::new(factory)));
    install_imgui_pump_with_cache(app, textures.clone());
    textures
}

/// Install a fresh imgui pump that shares an existing texture cache.
/// Used by the editor so previewer com_ids registered during the
/// welcome page remain valid after the main-editor pump replaces it.
pub fn install_imgui_pump_with_cache(
    app: &ComRc<IApplication>,
    textures: Rc<RefCell<ImguiTextureCache>>,
) {
    // Register the IImmediateDirector ProtoSpec before any script
    // wrap might surface a struct conforming to it. The fat-CCW
    // wrap path consults the registry to decide whether a slot is
    // backable; without this, `struct[IImmediateDirector] X`
    // wrapped as IDirector would QI-miss on IImmediateDirector and
    // silently disable `render_im`.
    register_immediate_director_proto();

    let engine_rc = app.engine();
    let engine = engine_rc.borrow();
    let ui_host: ComRc<IUiHost> = ImguiUiHost::create();
    let pump = Rc::new(ImguiImmediateDirectorPump::new(
        engine.ui_manager(),
        textures,
        ui_host,
    ));
    engine.clear_immediate_director_pump();
    engine.set_immediate_director_pump(pump);
}
