use std::num::NonZero;
use std::sync::{Arc, Mutex, RwLock};

use image::RgbaImage;
use lru::LruCache;

pub trait Texture: downcast_rs::Downcast {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

downcast_rs::impl_downcast!(Texture);

/// Coarse description of a texture's alpha channel, used by loaders that
/// can't otherwise tell whether a material should be opaque, alpha-tested
/// (binary cutout), or alpha-blended (translucent). Computed once at
/// texture-load time and cached on `TextureDef`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AlphaKind {
    Opaque,
    Cutout,
    Blend,
}

/// Fraction of pixels that must carry mid-range alpha
/// (`32..=223`) before we call a texture `Blend` rather than `Cutout`.
/// DDS BC1/BC3 decoders introduce a small amount of intermediate alpha
/// at the boundaries of binary-alpha source textures, but those edge
/// pixels cluster very close to `0` and `255` (the 1..32 and 224..254
/// bands). Counting only pixels squarely in the mid range cleanly
/// separates truly translucent surfaces (cloth, fog, glass — typical
/// graded scores of 0.4–1.0) from mostly-binary cutout atlases (PAL4
/// `wujian-*` interior props — typical graded scores below 0.07).
///
/// Without this guard, mostly-opaque cutout atlases end up rendered
/// with depth-write off, alpha-0 atlas texels stop being discarded, and
/// the next opaque draw behind them shows through the surface — the
/// "see through the table via the cloth's alpha" symptom on PAL4
/// indoor scenes.
const BLEND_PIXEL_FRACTION: f32 = 0.07;

pub struct TextureDef {
    name: String,
    /// The decoded CPU-side image. Backends consume this exactly once,
    /// at GPU upload time, via [`TextureDef::take_image`]; afterwards
    /// the `RgbaImage` is dropped and the slot returns `None`. PAL3
    /// scenes routinely reference enough textures that keeping the CPU
    /// copy alive in `TEXTURE_STORE` indefinitely costs tens of MB of
    /// otherwise-dead memory.
    ///
    /// Backends that don't cache their GPU texture object per
    /// `TextureDef` (e.g. `vitagl`) should keep using [`with_image`]
    /// instead of [`take_image`] so subsequent material creations on
    /// the same `TextureDef` still find the bytes. The Vulkan backend
    /// is safe to drain because `VulkanTextureStore` already caches
    /// `Rc<VulkanTexture>` by name, so the upload runs at most once
    /// per `TextureDef`.
    image: Mutex<Option<RgbaImage>>,
    alpha_kind: AlphaKind,
}

impl TextureDef {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Run `f` with a borrowed reference to the cached CPU-side
    /// `RgbaImage`, or `None` if the slot has already been drained
    /// (see [`take_image`]) or was never populated.
    pub fn with_image<R>(&self, f: impl FnOnce(Option<&RgbaImage>) -> R) -> R {
        let guard = self.image.lock().unwrap();
        f(guard.as_ref())
    }

    /// Drain the cached `RgbaImage` out of the `TextureDef`, freeing
    /// its memory. Returns `None` on the second call (or if no image
    /// was ever set).
    pub fn take_image(&self) -> Option<RgbaImage> {
        self.image.lock().unwrap().take()
    }

    pub fn alpha_kind(&self) -> AlphaKind {
        self.alpha_kind
    }
}

fn classify_alpha(image: Option<&RgbaImage>) -> AlphaKind {
    let Some(img) = image else {
        return AlphaKind::Opaque;
    };

    let mut graded_count: u64 = 0; // alpha in 32..=223
    let mut non_opaque_count: u64 = 0; // alpha < 255
    let mut total: u64 = 0;
    for px in img.pixels() {
        total += 1;
        let a = px.0[3];
        if a == 255 {
            continue;
        }
        non_opaque_count += 1;
        if (32..=223).contains(&a) {
            graded_count += 1;
        }
    }

    if total == 0 {
        return AlphaKind::Opaque;
    }

    if (graded_count as f32) / (total as f32) >= BLEND_PIXEL_FRACTION {
        AlphaKind::Blend
    } else if non_opaque_count > 0 {
        AlphaKind::Cutout
    } else {
        AlphaKind::Opaque
    }
}

/// Premultiply `image`'s RGB by its alpha channel in place.
///
/// Most authoring tools leave the RGB at black (or some other dark value)
/// in fully-transparent pixels of an RGBA texture. When such a texture is
/// rendered with straight-alpha blending and bilinear filtering, those
/// dark RGB values bleed into the surrounding semi-transparent texels and
/// the result is a black halo around translucent edges. Storing the
/// texture *premultiplied* (`rgb' = rgb * a/255`) and switching the
/// `AlphaBlend` color factor to `ONE / ONE_MINUS_SRC_ALPHA` (handled in
/// `pipeline.rs`) restores correct filtering: an averaged texel still
/// satisfies the premultiplied invariant so blending is hue-correct.
///
/// We only premultiply when the texture actually carries transparency
/// (`AlphaKind::Cutout` / `AlphaKind::Blend`); fully opaque textures
/// (`alpha == 255` for every pixel — including lightmaps with a junk
/// alpha channel of 255) are skipped, so opaque draws and `LightMap`
/// materials are bit-identical to before.
fn premultiply_alpha(image: &mut RgbaImage) {
    for px in image.pixels_mut() {
        let a = px.0[3] as u16;
        if a == 255 {
            continue;
        }
        // Round-to-nearest integer divide by 255 (equivalent to
        // `(x * a + 127) / 255` to within 1 ulp without a division).
        px.0[0] = ((px.0[0] as u16 * a + 127) / 255) as u8;
        px.0[1] = ((px.0[1] as u16 * a + 127) / 255) as u8;
        px.0[2] = ((px.0[2] as u16 * a + 127) / 255) as u8;
    }
}

lazy_static::lazy_static! {
    static ref TEXTURE_STORE: RwLock<LruCache<String, Arc<TextureDef>>> = RwLock::new(LruCache::new(NonZero::new(100).unwrap()));
}

pub struct TextureStore;
impl TextureStore {
    pub fn get_or_update(
        name: &str,
        update: impl FnOnce() -> Option<RgbaImage>,
    ) -> Arc<TextureDef> {
        let mut store = TEXTURE_STORE.write().unwrap();

        if let Some(t) = store.get(name) {
            t.clone()
        } else {
            let mut image = update();
            let alpha_kind = classify_alpha(image.as_ref());
            if alpha_kind != AlphaKind::Opaque {
                if let Some(img) = image.as_mut() {
                    premultiply_alpha(img);
                }
            }
            let t = Arc::new(TextureDef {
                name: name.to_string(),
                image: Mutex::new(image),
                alpha_kind,
            });
            store.put(name.to_string(), t.clone());
            t
        }
    }
}
