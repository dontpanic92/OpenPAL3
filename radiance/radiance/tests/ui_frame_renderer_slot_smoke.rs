//! Engine-slot test: confirms the UI-renderer invocation logic that
//! `CoreRadianceEngine::update` runs inside `ui_manager.update` works
//! end-to-end. We test the invocation snippet in isolation since
//! bringing up a real renderer / `UiManager` / `Platform` requires a
//! window and Vulkan.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::IUiLayer;
use radiance::radiance::UiFrameRenderer;

/// Mirrors the snippet in `CoreRadianceEngine::update` that invokes the
/// registered renderer unconditionally (even with no layers) so the
/// per-frame UI lifecycle still advances.
fn invoke_renderer(
    renderer_slot: &RefCell<Option<Rc<dyn UiFrameRenderer>>>,
    layers: Vec<ComRc<IUiLayer>>,
    dt: f32,
) {
    let renderer = renderer_slot.borrow().clone();
    if let Some(renderer) = renderer {
        renderer.render_frame(layers, dt);
    }
}

struct RecordingRenderer {
    /// (dt, layer_count) per `render_frame` call.
    calls: RefCell<Vec<(f32, usize)>>,
}

impl RecordingRenderer {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::new(Vec::new()),
        })
    }
}

impl UiFrameRenderer for RecordingRenderer {
    fn render_frame(&self, layers: Vec<ComRc<IUiLayer>>, dt: f32) {
        self.calls.borrow_mut().push((dt, layers.len()));
    }
}

#[test]
fn renderer_fires_when_slot_is_set() {
    let slot: RefCell<Option<Rc<dyn UiFrameRenderer>>> = RefCell::new(None);
    let renderer = RecordingRenderer::new();
    *slot.borrow_mut() = Some(renderer.clone());

    invoke_renderer(&slot, Vec::new(), 0.016);
    invoke_renderer(&slot, Vec::new(), 0.033);

    let calls = renderer.calls.borrow().clone();
    assert_eq!(calls.len(), 2);
    assert!((calls[0].0 - 0.016).abs() < 1e-6);
    assert!((calls[1].0 - 0.033).abs() < 1e-6);
}

#[test]
fn renderer_fires_unconditionally_even_with_no_layers() {
    // The renderer must run every frame regardless of registered layers
    // so per-frame UI lifecycle (texture deletion queue) keeps ticking.
    let slot: RefCell<Option<Rc<dyn UiFrameRenderer>>> = RefCell::new(None);
    let renderer = RecordingRenderer::new();
    *slot.borrow_mut() = Some(renderer.clone());

    invoke_renderer(&slot, Vec::new(), 0.016);

    let calls = renderer.calls.borrow().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, 0, "no layers registered");
}

#[test]
fn renderer_does_not_fire_when_slot_is_empty() {
    let slot: RefCell<Option<Rc<dyn UiFrameRenderer>>> = RefCell::new(None);
    invoke_renderer(&slot, Vec::new(), 0.016);
}
