//! Script-facing wrapper around a backend-allocated `RenderTarget`.
//!
//! Owns a `Box<dyn radiance::rendering::RenderTarget>` plus a stable
//! script-side `com_id` registered with the host's `ImguiTextureCache`.
//! Scripts call `target.texture_id()` to get the com_id and pass it to
//! `ui.image(...)`; the cache resolves it to the live `imgui::TextureId`
//! of the backing color image — which the wrapper re-registers on
//! resize so the same com_id stays valid for the target's lifetime.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use imgui::TextureId;
use radiance::rendering::RenderTarget as EngineRenderTarget;

use crate::comdef::services::{IRenderTarget, IRenderTargetImpl};
use crate::services::texture_cache::{next_handle_com_id, ImguiTextureCache};

pub struct ScriptedRenderTarget {
    inner: Rc<RefCell<Box<dyn EngineRenderTarget>>>,
    /// Independent queue handle, NOT the cache itself. Pushing to this
    /// on `Drop` is safe even while the cache is borrowed elsewhere
    /// (which is the case for the entire imgui frame — the cache is
    /// held in `borrow_mut()` by `ImguiImmediateDirectorPump::pump`,
    /// and GC can drop `ScriptedRenderTarget`s mid-frame).
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    com_id: i64,
}

ComObject_RenderTarget!(super::ScriptedRenderTarget);

impl ScriptedRenderTarget {
    pub fn create(
        target: Box<dyn EngineRenderTarget>,
        cache: Rc<RefCell<ImguiTextureCache>>,
    ) -> (ComRc<IRenderTarget>, Rc<RefCell<Box<dyn EngineRenderTarget>>>) {
        let com_id = next_handle_com_id();
        let tex_id = TextureId::new(target.imgui_texture_id() as usize);
        let pending_forgets = {
            let mut cache_ref = cache.borrow_mut();
            cache_ref.set_external(com_id, tex_id);
            cache_ref.pending_forgets_sink()
        };
        let inner = Rc::new(RefCell::new(target));
        let com = ComRc::from_object(Self {
            inner: Rc::clone(&inner),
            pending_forgets,
            com_id,
        });
        (com, inner)
    }
}

impl IRenderTargetImpl for ScriptedRenderTarget {
    fn width(&self) -> i32 {
        self.inner.borrow().extent().0 as i32
    }

    fn height(&self) -> i32 {
        self.inner.borrow().extent().1 as i32
    }

    fn resize(&self, w: i32, h: i32) {
        let w = w.max(1) as u32;
        let h = h.max(1) as u32;
        // The backend keeps `imgui_texture_id()` stable across resizes
        // (descriptor is `upsert`-replaced in place), so the
        // ImguiTextureCache entry registered in `create` remains valid
        // without any further bookkeeping.
        self.inner.borrow_mut().resize(w, h);
    }

    fn texture_id(&self) -> i32 {
        // `next_handle_com_id` hands out values that fit in i32 in
        // practice (single-process counter starting near 0), but i64 is
        // the underlying type. Truncate to i32 because the script-side
        // `ui.image(int, ...)` signature is i32; ImguiTextureCache
        // resolves the i32→i64 widening internally.
        self.com_id as i32
    }
}

impl Drop for ScriptedRenderTarget {
    fn drop(&mut self) {
        // Queue the com_id for cache removal. The cache drains its
        // pending queue at the start of every `&mut self` method, so
        // by the next imgui frame this entry is gone. We can't call
        // `cache.borrow_mut().forget(...)` directly here because drop
        // commonly fires from GC running inside the imgui frame, when
        // the cache is held in `borrow_mut()` by the frame state.
        self.pending_forgets.borrow_mut().push(self.com_id);
    }
}
