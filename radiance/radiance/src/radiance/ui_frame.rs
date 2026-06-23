//! Engine hook that drives the registered UI layers inside the imgui
//! frame scope.
//!
//! `CoreRadianceEngine` holds an `Option<Rc<dyn UiFrameRenderer>>` and
//! invokes it exactly once per [`CoreRadianceEngine::update`], inside the
//! active [`UiManager::update`] callback (so [`UiManager::ui()`] is live)
//! and *before* `scene_manager.update`, so a transition produced by a
//! layer's owning director first appears on the next frame.
//!
//! `radiance` owns the UI-layer abstraction ([`IUiLayer`]) but not the
//! imgui-rs `IUiHost` implementation (which lives in `radiance_scripting`
//! together with the texture cache). The renderer indirection keeps that
//! crate boundary: `radiance` collects the ordered layers and hands them
//! to the installed renderer, which sets up a single shared host frame,
//! ticks per-frame UI lifecycle (the texture deletion queue) and calls
//! `render` on each layer in order.
//!
//! [`CoreRadianceEngine::update`]: super::core_engine::CoreRadianceEngine::update
//! [`UiManager::update`]: super::ui_manager::UiManager::update
//! [`UiManager::ui()`]: super::ui_manager::UiManager::ui

use crosscom::ComRc;

use crate::comdef::IUiLayer;

pub trait UiFrameRenderer {
    /// Render one UI frame. Called once per [`CoreRadianceEngine::update`]
    /// inside the open imgui frame, unconditionally (even when `layers`
    /// is empty), so per-frame UI lifecycle (e.g. the frame-gated texture
    /// deletion queue) advances on every presented frame regardless of
    /// which director is active.
    ///
    /// Implementations set up a single shared
    /// [`IUiHost`](crate::comdef::IUiHost) / frame state and invoke
    /// `layer.render(host, dt)` on each entry in `layers`, which is
    /// already in render order (see [`UiLayerBand`](super::ui_layer::UiLayerBand)).
    ///
    /// [`CoreRadianceEngine::update`]: super::core_engine::CoreRadianceEngine::update
    fn render_frame(&self, layers: Vec<ComRc<IUiLayer>>, dt: f32);
}
