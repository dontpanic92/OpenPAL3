//! Generic UI sprite layer — the engine-level, game-agnostic
//! replacement for per-screen Rust "scene" objects.
//!
//! - [`SpriteService`] (`ISpriteService`) is a loader bound to a single
//!   vfs (e.g. a game's mounted `.cpk`). It decodes images and uploads
//!   them to the shared [`ImguiTextureCache`], handing back opaque
//!   handles.
//! - [`AtlasPage`] (`IAtlasPage`) is one uploaded texture page; its
//!   `sprite(x,y,w,h)` carves [`Sprite`] sub-rects from it.
//! - [`Sprite`] (`ISprite`) is the leaf handle a script caches once
//!   (`com_id` + size + UV) and feeds to `IUiHost.image_rect`/`image_uv`
//!   every frame.
//!
//! Lifetime: the uploaded page's com_id lives in an [`AtlasPageInner`]
//! shared (via `Rc`) by the page handle and every sprite it mints. When
//! the last of them drops, `AtlasPageInner::drop` pushes the com_id onto
//! the cache's `pending_forgets` sink, which the cache releases through
//! its **frame-gated deletion queue** (after `DELETION_GRACE_FRAMES`
//! presented frames) — so a handle dropped mid-frame never frees a
//! texture the GPU is still sampling.

use std::cell::RefCell;
use std::io::{BufReader, Read};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};

use crate::comdef::services::{
    IAtlasPage, IAtlasPageImpl, ISprite, ISpriteImpl, ISpriteService, ISpriteServiceImpl,
};
use crate::services::texture_cache::{ImguiTextureCache, next_handle_com_id};

/// Shared backing of an uploaded texture page. Holds the cache com_id
/// and releases it (frame-gated) when the last owner — the `AtlasPage`
/// handle plus every `Sprite` carved from it — drops.
struct AtlasPageInner {
    com_id: i64,
    width: i32,
    height: i32,
    pending_forgets: Rc<RefCell<Vec<i64>>>,
}

impl Drop for AtlasPageInner {
    fn drop(&mut self) {
        self.pending_forgets.borrow_mut().push(self.com_id);
    }
}

/// `ISprite` — a (possibly sub-rect) view onto an uploaded page. Holds a
/// strong `Rc` to the page so the page stays uploaded for the sprite's
/// lifetime.
pub struct Sprite {
    page: Rc<AtlasPageInner>,
    width: i32,
    height: i32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
}

ComObject_Sprite!(super::Sprite);

impl Sprite {
    fn create(
        page: Rc<AtlasPageInner>,
        width: i32,
        height: i32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) -> ComRc<ISprite> {
        ComRc::from_object(Self {
            page,
            width,
            height,
            u0,
            v0,
            u1,
            v1,
        })
    }
}

impl ISpriteImpl for Sprite {
    fn com_id(&self) -> i32 {
        self.page.com_id as i32
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn u0(&self) -> f32 {
        self.u0
    }
    fn v0(&self) -> f32 {
        self.v0
    }
    fn u1(&self) -> f32 {
        self.u1
    }
    fn v1(&self) -> f32 {
        self.v1
    }
}

/// `IAtlasPage` — an uploaded texture page sprites are carved from.
pub struct AtlasPage {
    inner: Rc<AtlasPageInner>,
}

ComObject_AtlasPage!(super::AtlasPage);

impl AtlasPage {
    fn from_inner(inner: Rc<AtlasPageInner>) -> ComRc<IAtlasPage> {
        ComRc::from_object(Self { inner })
    }
}

impl IAtlasPageImpl for AtlasPage {
    fn width(&self) -> i32 {
        self.inner.width
    }
    fn height(&self) -> i32 {
        self.inner.height
    }

    fn sprite(&self, x: i32, y: i32, w: i32, h: i32) -> ComRc<ISprite> {
        let (u0, v0, u1, v1) = sub_rect_uv(self.inner.width, self.inner.height, x, y, w, h);
        Sprite::create(self.inner.clone(), w.max(0), h.max(0), u0, v0, u1, v1)
    }

    fn whole(&self) -> ComRc<ISprite> {
        // Whole-page sprite: full [0,1] UV with NO half-texel inset (no
        // neighbouring sub-rect to bleed from), so single full images
        // render edge-to-edge.
        Sprite::create(
            self.inner.clone(),
            self.inner.width,
            self.inner.height,
            0.0,
            0.0,
            1.0,
            1.0,
        )
    }
}

/// `ISpriteService` — vfs-bound loader. Decodes + uploads images into the
/// shared texture cache and wraps them as sprite handles.
pub struct SpriteService {
    vfs: Rc<MiniFs>,
    cache: Rc<RefCell<ImguiTextureCache>>,
}

ComObject_SpriteService!(super::SpriteService);

impl SpriteService {
    pub fn create(
        vfs: Rc<MiniFs>,
        cache: Rc<RefCell<ImguiTextureCache>>,
    ) -> ComRc<ISpriteService> {
        ComRc::from_object(Self { vfs, cache })
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let file = self.vfs.open(path).ok()?;
        let mut bytes = Vec::new();
        BufReader::new(file).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    /// Read + decode + upload `path`, returning the shared page backing.
    /// Decoding mirrors the rest of the loaders: `image::load_from_memory`
    /// with a headerless-TGA fallback, plus the D3D9 bottom-up `.dds`
    /// vertical flip.
    fn upload(&self, path: &str) -> Option<Rc<AtlasPageInner>> {
        let bytes = match self.read(path) {
            Some(b) => b,
            None => {
                log::warn!("SpriteService: cannot read {}", path);
                return None;
            }
        };
        let decoded = image::load_from_memory(&bytes)
            .or_else(|_| image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga));
        let dyn_image = match decoded {
            Ok(img) => img,
            Err(e) => {
                log::warn!("SpriteService: {} decode failed: {:#}", path, e);
                return None;
            }
        };
        let dyn_image = if path.to_ascii_lowercase().ends_with(".dds") {
            dyn_image.flipv()
        } else {
            dyn_image
        };
        let rgba = dyn_image.to_rgba8();
        let (w, h) = rgba.dimensions();
        let com_id = next_handle_com_id();
        let pending_forgets = {
            let mut cache = self.cache.borrow_mut();
            if cache.upload_pixels(com_id, &rgba.into_raw(), w, h).is_none() {
                log::warn!("SpriteService: {} upload failed (com_id={})", path, com_id);
                return None;
            }
            cache.pending_forgets_sink()
        };
        Some(Rc::new(AtlasPageInner {
            com_id,
            width: w as i32,
            height: h as i32,
            pending_forgets,
        }))
    }
}

impl ISpriteServiceImpl for SpriteService {
    fn load_sprite(&self, vfs_path: &str) -> Option<ComRc<ISprite>> {
        let inner = self.upload(vfs_path)?;
        Some(Sprite::create(
            inner.clone(),
            inner.width,
            inner.height,
            0.0,
            0.0,
            1.0,
            1.0,
        ))
    }

    fn load_atlas_page(&self, vfs_path: &str) -> Option<ComRc<IAtlasPage>> {
        let inner = self.upload(vfs_path)?;
        Some(AtlasPage::from_inner(inner))
    }
}

/// Normalised UV for a pixel sub-rect, with a half-texel inset on every
/// edge to prevent atlas bleeding (the GPU's bilinear sampler at the
/// exact boundary otherwise picks up neighbouring atlas pixels). Mirrors
/// the legacy `Pal3StartMenuScene::resolve_sprite`.
fn sub_rect_uv(page_w: i32, page_h: i32, x: i32, y: i32, w: i32, h: i32) -> (f32, f32, f32, f32) {
    let pw = page_w.max(1) as f32;
    let ph = page_h.max(1) as f32;
    let inset_u = 0.5 / pw;
    let inset_v = 0.5 / ph;
    let u0 = x as f32 / pw + inset_u;
    let v0 = y as f32 / ph + inset_v;
    let u1 = (x + w) as f32 / pw - inset_u;
    let v1 = (y + h) as f32 / ph - inset_v;
    (u0, v0, u1, v1)
}

#[cfg(test)]
mod tests {
    use super::sub_rect_uv;

    #[test]
    fn sub_rect_applies_half_texel_inset() {
        // 256x256 page, the top-left 64x64 tile.
        let (u0, v0, u1, v1) = sub_rect_uv(256, 256, 0, 0, 64, 64);
        let texel = 0.5 / 256.0;
        assert!((u0 - texel).abs() < 1e-6);
        assert!((v0 - texel).abs() < 1e-6);
        assert!((u1 - (64.0 / 256.0 - texel)).abs() < 1e-6);
        assert!((v1 - (64.0 / 256.0 - texel)).abs() < 1e-6);
    }

    #[test]
    fn sub_rect_offset_tile() {
        // A 32x16 tile at (64, 128) of a 128x256 page.
        let (u0, v0, u1, v1) = sub_rect_uv(128, 256, 64, 128, 32, 16);
        assert!((u0 - (64.0 / 128.0 + 0.5 / 128.0)).abs() < 1e-6);
        assert!((v0 - (128.0 / 256.0 + 0.5 / 256.0)).abs() < 1e-6);
        assert!((u1 - (96.0 / 128.0 - 0.5 / 128.0)).abs() < 1e-6);
        assert!((v1 - (144.0 / 256.0 - 0.5 / 256.0)).abs() < 1e-6);
    }

    #[test]
    fn degenerate_page_size_does_not_divide_by_zero() {
        let (u0, v0, u1, v1) = sub_rect_uv(0, 0, 0, 0, 0, 0);
        for c in [u0, v0, u1, v1] {
            assert!(c.is_finite());
        }
    }
}

