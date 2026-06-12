//! `VideoHandle` — `IVideoHandle` impl shared by the editor preview
//! tab and the PAL3 menu intro-movie path. Wraps a `Box<VideoPlayer>`
//! (engine factory) and routes per-frame `texture_com_id()` queries
//! through an `ImguiTextureCache` slot, just like the editor's image
//! and audio handles.
//!
//! Class declared in `crosscom/idl/shared_services.idl` so the
//! `ComObject_VideoHandle!` macro generates here in `shared`.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::rendering::VideoPlayer;
use radiance::video::VideoStreamState;
use radiance_scripting::comdef::services::{IVideoHandle, IVideoHandleImpl};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::services::texture_cache::next_handle_com_id;

pub struct VideoHandle {
    player: RefCell<Box<VideoPlayer>>,
    width: u32,
    height: u32,
    com_id: i64,
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    /// Sink for per-frame texture-id updates. Pushed from
    /// `texture_com_id` so the cache adopts the latest `TextureId`
    /// on its next `&mut self` (which always precedes a `resolve` —
    /// see `ImguiTextureCache::pending_external_updates`). Borrowing
    /// the outer `Rc<RefCell<ImguiTextureCache>>` from
    /// `texture_com_id` panics: the imgui pump holds it in
    /// `borrow_mut()` for the whole frame.
    pending_external_updates: Rc<RefCell<Vec<(i64, imgui::TextureId)>>>,
    last_texture: RefCell<Option<imgui::TextureId>>,
}

ComObject_VideoHandle!(super::VideoHandle);

impl VideoHandle {
    pub fn create(
        cache: Rc<RefCell<ImguiTextureCache>>,
        player: Box<VideoPlayer>,
        width: u32,
        height: u32,
    ) -> ComRc<IVideoHandle> {
        let com_id = next_handle_com_id();
        let (pending_forgets, pending_external_updates) = {
            let c = cache.borrow();
            (c.pending_forgets_sink(), c.pending_external_updates_sink())
        };
        ComRc::from_object(Self {
            player: RefCell::new(player),
            width,
            height,
            com_id,
            pending_forgets,
            pending_external_updates,
            last_texture: RefCell::new(None),
        })
    }
}

impl Drop for VideoHandle {
    fn drop(&mut self) {
        // See `ImageHandle::Drop` — queue the cache removal because
        // GC may fire this destructor mid-imgui-frame while the cache
        // is held in `borrow_mut()` by the frame state.
        self.pending_forgets.borrow_mut().push(self.com_id);
    }
}

impl IVideoHandleImpl for VideoHandle {
    fn toggle(&self) {
        let mut player = self.player.borrow_mut();
        if player.get_state() == VideoStreamState::Playing {
            player.pause();
        } else {
            player.resume();
        }
    }

    fn state(&self) -> i32 {
        match self.player.borrow().get_state() {
            VideoStreamState::Stopped => 0,
            VideoStreamState::Playing => 1,
            VideoStreamState::Paused => 2,
        }
    }

    fn width(&self) -> i32 {
        self.width as i32
    }

    fn height(&self) -> i32 {
        self.height as i32
    }

    fn texture_com_id(&self) -> i32 {
        // Pull a fresh imgui TextureId from the player every frame.
        // We can't borrow `cache` here — the imgui pump holds it in
        // `borrow_mut()` for the whole frame — so we push the mapping
        // into the cache's external-update queue; the cache drains it
        // on the next `&mut self` access, which always precedes a
        // `resolve(com_id)` from `image*` widgets.
        let mut last = self.last_texture.borrow_mut();
        if let Some(id) = self.player.borrow_mut().get_texture(*last) {
            if Some(id) != *last {
                self.pending_external_updates
                    .borrow_mut()
                    .push((self.com_id, id));
                *last = Some(id);
            }
        }
        self.com_id as i32
    }

    fn duration_ms(&self) -> i32 {
        // Saturate to i32 — UI bar uses i32 anyway and no preview clip
        // approaches the 2.1B ms (~24-day) ceiling.
        self.player.borrow().duration_ms().clamp(0, i32::MAX as i64) as i32
    }

    fn position_ms(&self) -> i32 {
        self.player.borrow().position_ms().clamp(0, i32::MAX as i64) as i32
    }

    fn seek_ms(&self, ms: i32) {
        self.player.borrow_mut().seek_ms(ms.max(0) as i64);
    }

    fn restart(&self) {
        self.player.borrow_mut().restart();
    }

    fn stop(&self) {
        self.player.borrow_mut().stop();
    }

    fn set_looping(&self, looping: bool) {
        self.player.borrow_mut().set_looping(looping);
    }

    fn looping(&self) -> bool {
        self.player.borrow().looping()
    }
}
