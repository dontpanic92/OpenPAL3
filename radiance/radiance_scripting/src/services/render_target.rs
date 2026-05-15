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
    cache: Rc<RefCell<ImguiTextureCache>>,
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
        cache.borrow_mut().set_external(com_id, tex_id);
        let inner = Rc::new(RefCell::new(target));
        let com = ComRc::from_object(Self {
            inner: Rc::clone(&inner),
            cache,
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
        let new_id = {
            let mut t = self.inner.borrow_mut();
            t.resize(w, h);
            TextureId::new(t.imgui_texture_id() as usize)
        };
        // Re-bind the same com_id to the new TextureId so scripts don't
        // need to re-fetch after a resize.
        self.cache.borrow_mut().set_external(self.com_id, new_id);
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
        self.cache.borrow_mut().forget(self.com_id);
    }
}
