//! Engine-side hook that drives an immediate-mode director's
//! `render_im` (or equivalent) call inside the imgui frame scope.
//!
//! `CoreRadianceEngine` holds an `Option<Rc<dyn ImmediateDirectorPump>>`
//! and invokes it exactly once per [`CoreRadianceEngine::update`],
//! immediately after `scene_manager.update(dt)` and *inside* the
//! active [`UiManager::update`] callback (so [`UiManager::ui()`] is
//! live for the pump implementation to read).
//!
//! `radiance` does not know about `IImmediateDirector` (it lives in
//! the `radiance_scripting` crate). Implementors of this trait are
//! expected to QI the supplied [`ComRc<IDirector>`] for their target
//! interface and silently skip if the QI fails. This keeps the crate
//! boundary clean and lets multiple pumps coexist (only one is
//! installed at a time today; the API does not preclude future
//! generalisation to a Vec).
//!
//! [`CoreRadianceEngine::update`]: super::core_engine::CoreRadianceEngine::update

use crosscom::ComRc;

use crate::comdef::IDirector;

pub trait ImmediateDirectorPump {
    /// Drive whatever per-frame immediate-mode work the engine's
    /// currently-active director needs while imgui is mid-frame.
    /// Implementations typically:
    ///
    /// 1. `query_interface::<IImmediateDirector>` (or similar) on
    ///    `director`; return early if the QI fails (the active
    ///    director isn't immediate-mode).
    /// 2. Park frame-state (textures, fonts, dpi) appropriately.
    /// 3. Call the script-facing `render_im` method.
    fn pump(&self, director: ComRc<IDirector>, dt: f32);
}
