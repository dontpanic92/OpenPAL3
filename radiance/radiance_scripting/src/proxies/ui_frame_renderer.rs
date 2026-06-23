//! Production [`UiFrameRenderer`] backed by `imgui-rs`. Installed on
//! `CoreRadianceEngine` via
//! [`radiance::radiance::CoreRadianceEngine::set_ui_frame_renderer`].
//!
//! Per-frame work (once per [`CoreRadianceEngine::update`], inside the
//! engine's open imgui frame):
//! 1. Advance the frame-gated texture-deletion queue (runs every frame,
//!    regardless of which layers are registered).
//! 2. Park a single [`ImguiFrameState`](crate::services::ui_host::ImguiFrameState)
//!    around the whole layer pass so `ImguiUiHost` can issue real
//!    imgui-rs calls.
//! 3. Invoke `render(ui_host, dt)` on each layer in `layers` (already in
//!    render/z-order), then submit the perf overlay last so it draws on
//!    top.
//!
//! [`CoreRadianceEngine::update`]: radiance::radiance::CoreRadianceEngine::update

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IUiHost, IUiLayer};
use radiance::radiance::{UiFrameRenderer, UiManager};

use crate::services::ui_host::{ImguiFrameState, with_imgui_frame};
use crate::services::{ImguiTextureCache, PerfOverlay};

pub struct ImguiUiFrameRenderer {
    ui_manager: Rc<UiManager>,
    textures: Rc<RefCell<ImguiTextureCache>>,
    ui_host: ComRc<IUiHost>,
    perf_overlay: PerfOverlay,
}

impl ImguiUiFrameRenderer {
    pub fn new(
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
        ui_host: ComRc<IUiHost>,
    ) -> Self {
        Self {
            ui_manager,
            textures,
            ui_host,
            perf_overlay: PerfOverlay::new(),
        }
    }

    /// Headless dispatch: invoke a single layer's `render` against a
    /// caller-supplied `IUiHost` without parking an imgui frame state.
    /// Useful for tests that drive a layer with a recording `IUiHost`
    /// and do not need a real imgui context.
    pub fn dispatch_render(layer: &ComRc<IUiLayer>, ui_host: &ComRc<IUiHost>, dt: f32) {
        layer.render(ui_host.clone(), dt);
    }
}

impl UiFrameRenderer for ImguiUiFrameRenderer {
    fn render_frame(&self, layers: Vec<ComRc<IUiLayer>>, dt: f32) {
        let frame_start = std::time::Instant::now();

        // Once-per-presented-frame tick of the frame-gated texture
        // deletion queue. Runs every frame regardless of registered
        // layers, so games whose active director is not a UI layer
        // (e.g. PAL3's AdventureDirector) still release dropped UI
        // textures. Borrow is short-lived and dropped before the
        // frame-state borrow below.
        self.textures.borrow_mut().advance_frame();

        let ui = self.ui_manager.ui();
        let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
        let mut tex = self.textures.borrow_mut();
        let mut frame = ImguiFrameState::new(ui, &mut *tex, &fonts, self.ui_manager.dpi_scale());
        with_imgui_frame(&mut frame, || {
            for layer in &layers {
                let render_start = std::time::Instant::now();
                layer.render(self.ui_host.clone(), dt);
                radiance::perf::count(
                    "ui.layer_render_total_ns",
                    render_start.elapsed().as_nanos() as u64,
                );
            }
            // Perf overlay submits last so it draws on top of every
            // layer. No-op when the env flag isn't set.
            self.perf_overlay.render(ui);
        });
        radiance::perf::count(
            "ui.render_frame_total_ns",
            frame_start.elapsed().as_nanos() as u64,
        );
    }
}
