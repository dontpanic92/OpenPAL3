use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use crosscom::ComRc;
use imgui::TextureId;
use radiance::rendering::{ComponentFactory, Texture as RenderingTexture};

use crate::comdef::services::ITexture;
use crate::services::texture_resolver::TextureResolver;

use super::texture::Texture;

struct CachedTexture {
    _texture: Option<Box<dyn RenderingTexture>>,
    id: TextureId,
}

pub struct ImguiTextureCache {
    factory: Rc<dyn ComponentFactory>,
    cache: HashMap<i64, CachedTexture>,
    /// Com-ids queued for removal by destructors that ran while the
    /// cache was already borrowed elsewhere (e.g.
    /// `ScriptedRenderTarget::Drop` firing from GC inside an
    /// `ImguiFrameState`-parked render pass — see plan.md follow-up
    /// #5). These are NOT evicted immediately: they are moved into the
    /// frame-gated `deletion_queue` on the next `&mut self` access, so a
    /// texture dropped mid-frame is not freed while the GPU may still be
    /// sampling it this frame.
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    /// External texture-id updates queued by handles that produce a
    /// fresh `imgui::TextureId` every frame (e.g. `VideoHandle`) and
    /// therefore can't borrow the cache directly from inside an
    /// active imgui frame — the pump holds the outer
    /// `Rc<RefCell<ImguiTextureCache>>` in `borrow_mut()` for the
    /// whole frame, so any nested `cache.borrow_mut()` would panic.
    /// Producers push `(com_id, texture_id)` here from their COM
    /// methods; the cache drains the queue on its next `&mut self`
    /// access — which always precedes a `resolve(com_id)` because
    /// `image*` widgets call `resolve` through the same exclusive
    /// borrow held by the pump.
    pending_external_updates: Rc<RefCell<Vec<(i64, TextureId)>>>,
    /// Monotonic presented-frame counter, advanced once per frame by
    /// [`advance_frame`](Self::advance_frame). Drives the frame-gated
    /// deletion queue.
    frame_counter: u64,
    /// Frame-gated GPU deletion queue: `(com_id, release_frame)`. A
    /// forgotten texture's cache entry (and its owned GPU resource) is
    /// evicted only once `frame_counter >= release_frame`, i.e. after
    /// [`DELETION_GRACE_FRAMES`] presented frames, mirroring an engine's
    /// fence-gated deferred-deletion queue. Com-ids are never reused
    /// (`next_handle_com_id` hands out monotonically-decreasing
    /// negatives), so an id sitting here can never collide with a fresh
    /// upload.
    deletion_queue: Vec<(i64, u64)>,
}

/// Number of presented frames a forgotten texture lingers before its GPU
/// resource is actually freed. Must be `>= MAX_FRAMES_IN_FLIGHT` so no
/// in-flight frame still references the texture when it is dropped; we
/// use `MAX_FRAMES_IN_FLIGHT + 1` (= 3, with `MAX_FRAMES_IN_FLIGHT = 2`
/// in the vulkan backend) for a one-frame safety margin and to stay
/// correct on backends that don't expose that constant.
const DELETION_GRACE_FRAMES: u64 = 3;

impl ImguiTextureCache {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            factory,
            cache: HashMap::new(),
            pending_forgets: Rc::new(RefCell::new(Vec::new())),
            pending_external_updates: Rc::new(RefCell::new(Vec::new())),
            frame_counter: 0,
            deletion_queue: Vec::new(),
        }
    }

    /// Shared queue handle. Destructors that can't safely call
    /// `forget()` directly (because the cache is borrowed elsewhere)
    /// should hold an `Rc` clone of this and push their com_id on
    /// drop. The cache picks the queued ids up on its next `&mut`
    /// access.
    pub fn pending_forgets_sink(&self) -> Rc<RefCell<Vec<i64>>> {
        self.pending_forgets.clone()
    }

    /// Shared queue handle for `set_external`-equivalent updates that
    /// must happen mid-frame (when the cache is borrowed by the
    /// imgui pump). See `pending_external_updates` field doc.
    pub fn pending_external_updates_sink(&self) -> Rc<RefCell<Vec<(i64, TextureId)>>> {
        self.pending_external_updates.clone()
    }

    fn drain_pending(&mut self) {
        // Move newly-queued forgets (pushed by Drop handlers) into the
        // frame-stamped deletion queue rather than evicting them now.
        // Stamping with `frame_counter + GRACE` defers the actual GPU
        // free until enough presented frames have elapsed that no
        // in-flight frame still references the texture.
        let release_frame = self.frame_counter + DELETION_GRACE_FRAMES;
        let to_defer: Vec<i64> = self.pending_forgets.borrow_mut().drain(..).collect();
        for id in to_defer {
            self.deletion_queue.push((id, release_frame));
        }

        // Evict entries whose grace period has elapsed. swap_remove is
        // O(1); deletion order doesn't matter. Removing the cache entry
        // drops its owned GPU texture (`CachedTexture::_texture`).
        let current = self.frame_counter;
        let mut i = 0;
        while i < self.deletion_queue.len() {
            if self.deletion_queue[i].1 <= current {
                let (com_id, _) = self.deletion_queue.swap_remove(i);
                self.cache.remove(&com_id);
            } else {
                i += 1;
            }
        }

        let to_update: Vec<(i64, TextureId)> = self
            .pending_external_updates
            .borrow_mut()
            .drain(..)
            .collect();
        for (com_id, texture_id) in to_update {
            self.cache.insert(
                com_id,
                CachedTexture {
                    _texture: None,
                    id: texture_id,
                },
            );
        }
    }

    fn drain_pending_forgets(&mut self) {
        self.drain_pending();
    }

    /// Advance the presented-frame counter by one and process the
    /// frame-gated deletion queue. MUST be called exactly once per
    /// presented frame, from a director-agnostic hook (the PAL3
    /// adventure director is not an `IUiLayer`, so the imgui
    /// pump alone does not tick every frame). Idempotent w.r.t. cache
    /// state otherwise — it is safe for `drain_pending` to also run on
    /// every `&mut self` access between ticks, since only `advance_frame`
    /// moves `frame_counter`.
    pub fn advance_frame(&mut self) {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.drain_pending();
    }

    pub fn upload(&mut self, com_id: i64, tex: ComRc<ITexture>) -> Option<TextureId> {
        self.drain_pending_forgets();
        if let Some(entry) = self.cache.get(&com_id) {
            return Some(entry.id);
        }

        let tex = concrete_texture(&tex);
        let (width, height) = tex.extent();
        let pixels = tex.pixels();
        self.upload_pixels(com_id, pixels, width, height)
    }

    /// Uploads `pixels` (RGBA8) directly under the given `com_id`. If
    /// `com_id` is already known, returns the cached id without re-uploading.
    pub fn upload_pixels(
        &mut self,
        com_id: i64,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Option<TextureId> {
        self.drain_pending_forgets();
        if let Some(entry) = self.cache.get(&com_id) {
            return Some(entry.id);
        }

        if width == 0 || height == 0 || pixels.len() < width as usize * height as usize * 4 {
            log::warn!(
                "refusing to upload invalid scripted texture: com_id={}, extent={}x{}, bytes={}",
                com_id,
                width,
                height,
                pixels.len()
            );
            return None;
        }

        let uploaded = catch_unwind(AssertUnwindSafe(|| {
            self.factory
                .create_imgui_texture(pixels, 0, width, height, None)
        }));

        match uploaded {
            Ok((texture, id)) => {
                self.cache.insert(
                    com_id,
                    CachedTexture {
                        _texture: Some(texture),
                        id,
                    },
                );
                Some(id)
            }
            Err(_) => {
                log::warn!("failed to upload scripted texture: com_id={}", com_id);
                None
            }
        }
    }

    /// Records `texture_id` as the imgui texture for `com_id`. The cache does
    /// NOT take ownership of the underlying GPU texture — used by external
    /// owners (e.g. a video player) that recycle the same TextureId across
    /// frames. Re-calling overwrites the mapping.
    pub fn set_external(&mut self, com_id: i64, texture_id: TextureId) {
        self.drain_pending_forgets();
        self.cache.insert(
            com_id,
            CachedTexture {
                _texture: None,
                id: texture_id,
            },
        );
    }

    /// Drops the cached entry (and the owned GPU texture, if any) for
    /// `com_id`. Subsequent `resolve` calls will return None.
    pub fn forget(&mut self, com_id: i64) {
        self.drain_pending_forgets();
        self.cache.remove(&com_id);
    }

    pub fn resolve(&mut self, com_id: i64) -> Option<TextureId> {
        self.drain_pending_forgets();
        self.cache.get(&com_id).map(|entry| entry.id)
    }
}

impl TextureResolver for ImguiTextureCache {
    fn resolve(&mut self, com_id: i64) -> Option<TextureId> {
        self.resolve(com_id)
    }
}

/// Allocate a fresh com_id for handle types that need a stable cache key
/// without round-tripping through `ComObjectTable::intern`. Negative numbers
/// are used to keep the space disjoint from the positive ids the COM table
/// hands out.
pub fn next_handle_com_id() -> i64 {
    static NEXT: AtomicI64 = AtomicI64::new(0);
    -1 - NEXT.fetch_add(1, Ordering::Relaxed)
}

fn concrete_texture(tex: &ComRc<ITexture>) -> &Texture {
    let inner_offset = align_up(
        size_of::<ITexture>() + size_of::<AtomicU32>(),
        align_of::<Texture>(),
    );
    unsafe { &*((tex.ptr_value() as *const u8).add(inner_offset) as *const Texture) }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}
