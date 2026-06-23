//! Engine-owned UI layer stack.
//!
//! UI rendering is decoupled from the director lifecycle: any number of
//! [`IUiLayer`](crate::comdef::IUiLayer) surfaces can be registered, and
//! the engine drives every one of them once per frame inside its single
//! imgui frame scope, ordered by [`UiLayerBand`] (lower bands first /
//! underneath) and, within a band, registration order.
//!
//! Registration returns a [`UiLayerHandle`] whose `Drop` unregisters the
//! layer, so a surface stays alive exactly as long as its owner holds the
//! handle (e.g. a director registers in `activate`, drops in `deactivate`).
//!
//! The active director is *not* registered here; the engine auto-bridges
//! it into the [`UiLayerBand::Scene`] band each frame when it QIs to
//! `IUiLayer` (see `CoreRadianceEngine::update`).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crosscom::ComRc;

use crate::comdef::IUiLayer;

/// Z-order band for a registered UI layer. Lower discriminants render
/// first (underneath); higher ones draw on top.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum UiLayerBand {
    /// Base game / screen content (e.g. the active director's screen,
    /// auto-bridged here by the engine).
    Scene = 0,
    /// Persistent heads-up display drawn over the scene.
    Hud = 1,
    /// Modal-ish surfaces (dialog boxes, menus) above the HUD.
    Dialog = 2,
    /// Developer overlays (debug panels, perf counters) on top of all.
    DebugOverlay = 3,
}

struct Entry {
    id: u64,
    band: UiLayerBand,
    layer: ComRc<IUiLayer>,
}

/// Ordered set of registered UI layers. Owned by [`UiManager`].
#[derive(Default)]
pub struct UiLayerStack {
    entries: Vec<Entry>,
    next_id: u64,
}

impl UiLayerStack {
    fn register(&mut self, band: UiLayerBand, layer: ComRc<IUiLayer>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry { id, band, layer });
        id
    }

    fn unregister(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    /// Registered layers in render order: stable-sorted by band so
    /// within-band registration order is preserved.
    fn ordered(&self) -> Vec<ComRc<IUiLayer>> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        entries.sort_by_key(|e| e.band);
        entries.into_iter().map(|e| e.layer.clone()).collect()
    }
}

/// RAII registration handle. Dropping it unregisters the layer.
pub struct UiLayerHandle {
    stack: Weak<RefCell<UiLayerStack>>,
    id: u64,
}

impl UiLayerHandle {
    pub(crate) fn new(stack: &Rc<RefCell<UiLayerStack>>, id: u64) -> Self {
        Self {
            stack: Rc::downgrade(stack),
            id,
        }
    }
}

impl Drop for UiLayerHandle {
    fn drop(&mut self) {
        if let Some(stack) = self.stack.upgrade() {
            stack.borrow_mut().unregister(self.id);
        }
    }
}

/// Register `layer` into `stack` under `band` and return its handle.
pub(crate) fn register(
    stack: &Rc<RefCell<UiLayerStack>>,
    band: UiLayerBand,
    layer: ComRc<IUiLayer>,
) -> UiLayerHandle {
    let id = stack.borrow_mut().register(band, layer);
    UiLayerHandle::new(stack, id)
}

/// Registered layers of `stack` in render order.
pub(crate) fn ordered(stack: &Rc<RefCell<UiLayerStack>>) -> Vec<ComRc<IUiLayer>> {
    stack.borrow().ordered()
}
