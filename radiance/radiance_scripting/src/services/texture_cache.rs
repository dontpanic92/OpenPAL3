use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::{align_of, size_of};
use std::panic::{catch_unwind, AssertUnwindSafe};
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
    /// #5). Drained at the start of every `&mut self` method, so by
    /// the time any consumer observes the cache state, queued forgets
    /// have been applied.
    pending_forgets: Rc<RefCell<Vec<i64>>>,
}

impl ImguiTextureCache {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            factory,
            cache: HashMap::new(),
            pending_forgets: Rc::new(RefCell::new(Vec::new())),
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

    fn drain_pending_forgets(&mut self) {
        // Take ownership of the queued ids before iterating so handlers
        // that drop other handles re-entrantly can append to the queue
        // without aliasing the iterator.
        let to_forget: Vec<i64> = self.pending_forgets.borrow_mut().drain(..).collect();
        for id in to_forget {
            self.cache.remove(&id);
        }
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
