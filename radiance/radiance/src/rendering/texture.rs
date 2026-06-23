use std::num::NonZero;
use std::sync::{Arc, Mutex, RwLock};

use image::RgbaImage;
use lru::LruCache;

pub trait Texture {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

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

/// How [`TextureStore::get_or_update_inner`] treats a texture's alpha.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AlphaHandling {
    /// Detect transparency and premultiply RGB when present (default).
    Classify,
    /// Force alpha to 255 (opaque); never premultiply. For textures whose
    /// alpha is non-coverage detail data.
    ForceOpaque,
    /// Keep all channels verbatim; classify Opaque (never premultiply). For
    /// textures whose channels are arbitrary data (e.g. weight atlases).
    Raw,
}

/// Minimum fraction of mid-range-alpha (`32..=223`) pixels for a texture
/// to *qualify* as truly translucent (`Blend`). A genuinely see-through
/// surface — water, fog, glass, ghosts — carries graded alpha across a
/// large share of its area (PAL4 `y02` water ≈ 0.43, `yun02` cloud ≈
/// 0.85). A solid surface that merely has anti-aliased / cutout edges
/// carries only a sliver of graded pixels (PAL4 lotus `zjtai*` ≈
/// 0.001–0.013) and must NOT be treated as translucent — it has to keep
/// depth-write on so it self-occludes correctly.
///
/// The previous threshold (0.001) was far too low: it swept binary-alpha
/// cutout atlases with merely soft edges into the depth-write-off `Blend`
/// bucket. That both reintroduced the "see through the table via the
/// cloth's alpha" symptom AND, once translucent draws stopped writing
/// depth, destroyed self-occlusion within concave solid meshes (the PAL4
/// start-menu lotus, whose brown bowl composited in the wrong order).
const BLEND_GRADED_FRACTION_MIN: f32 = 0.05;

/// A texture that is predominantly fully-opaque (alpha == 255) is a solid
/// surface, never a translucent one, regardless of a few graded edge
/// texels. Genuine translucent surfaces have almost no fully-opaque
/// pixels (PAL4 `y02`/`yun02` ≈ 0.0 opaque), whereas the lotus cutout
/// atlases are 0.66–0.88 opaque. Requiring the opaque fraction to be
/// below this ceiling — in addition to [`BLEND_GRADED_FRACTION_MIN`] —
/// guards the classification from both directions.
const BLEND_OPAQUE_FRACTION_MAX: f32 = 0.5;

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

    let total_f = total as f32;
    let graded_fraction = graded_count as f32 / total_f;
    let opaque_fraction = (total - non_opaque_count) as f32 / total_f;

    // A surface is genuinely translucent (`Blend`, depth-write OFF) only
    // when graded alpha covers a substantial share of it AND it is not
    // predominantly solid. Solid surfaces with cutout holes or merely
    // soft anti-aliased edges fall through to `Cutout` (depth-write ON)
    // so they self-occlude; fully-opaque ones to `Opaque`.
    if graded_fraction >= BLEND_GRADED_FRACTION_MIN && opaque_fraction <= BLEND_OPAQUE_FRACTION_MAX
    {
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
        Self::get_or_update_inner(name, update, AlphaHandling::Classify)
    }

    /// Like [`TextureStore::get_or_update`], but treats the texture as
    /// fully opaque regardless of its alpha channel: every texel's alpha
    /// is forced to 255 before classification, so the texture is always
    /// `AlphaKind::Opaque` and its RGB is **never** premultiplied.
    ///
    /// Used for textures whose alpha channel is *not* coverage but carries
    /// unrelated data — e.g. PAL5 terrain `.dds`, whose DXT5 alpha holds
    /// detail/overlay information. Without this, `classify_alpha` would
    /// tag them `Blend`/`Cutout` and `premultiply_alpha` would darken
    /// their RGB (often to near-black), even though the material renders
    /// opaque.
    pub fn get_or_update_opaque(
        name: &str,
        update: impl FnOnce() -> Option<RgbaImage>,
    ) -> Arc<TextureDef> {
        Self::get_or_update_inner(name, update, AlphaHandling::ForceOpaque)
    }

    /// Like [`TextureStore::get_or_update`], but keeps the RGBA bytes
    /// **verbatim**: the texture is classified `AlphaKind::Opaque`, its
    /// alpha is left unchanged, and its RGB is never premultiplied.
    ///
    /// Used for textures whose channels are arbitrary *data* rather than a
    /// color + coverage — e.g. a PAL5 terrain splat **weight atlas** where
    /// R/G/B/A each hold a layer's blend weight. Both `get_or_update`
    /// (premultiply) and `get_or_update_opaque` (force alpha = 255) would
    /// corrupt such data; this path preserves every channel.
    pub fn get_or_update_raw(
        name: &str,
        update: impl FnOnce() -> Option<RgbaImage>,
    ) -> Arc<TextureDef> {
        Self::get_or_update_inner(name, update, AlphaHandling::Raw)
    }

    fn get_or_update_inner(
        name: &str,
        update: impl FnOnce() -> Option<RgbaImage>,
        alpha: AlphaHandling,
    ) -> Arc<TextureDef> {
        let mut store = TEXTURE_STORE.write().unwrap();

        if let Some(t) = store.get(name) {
            t.clone()
        } else {
            let mut image = update();
            let alpha_kind = match alpha {
                // Keep channels verbatim, classify Opaque (no premultiply).
                AlphaHandling::Raw => AlphaKind::Opaque,
                AlphaHandling::ForceOpaque => {
                    if let Some(img) = image.as_mut() {
                        for px in img.pixels_mut() {
                            px.0[3] = 255;
                        }
                    }
                    AlphaKind::Opaque
                }
                AlphaHandling::Classify => {
                    let kind = classify_alpha(image.as_ref());
                    if kind != AlphaKind::Opaque {
                        if let Some(img) = image.as_mut() {
                            premultiply_alpha(img);
                        }
                    }
                    kind
                }
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Build a 64x64 RGBA image whose alpha channel matches the requested
    /// pixel fractions: `opaque` pixels at alpha 255, `transparent` at 0,
    /// and the remainder at a mid-range "graded" value (128).
    fn image_with_alpha(opaque: f32, transparent: f32) -> RgbaImage {
        let (w, h) = (64u32, 64u32);
        let total = (w * h) as f32;
        let n_opaque = (opaque * total).round() as u32;
        let n_transparent = (transparent * total).round() as u32;
        let mut img = RgbaImage::new(w, h);
        let mut i = 0u32;
        for px in img.pixels_mut() {
            let a = if i < n_opaque {
                255
            } else if i < n_opaque + n_transparent {
                0
            } else {
                128
            };
            *px = Rgba([200, 180, 120, a]);
            i += 1;
        }
        img
    }

    #[test]
    fn fully_opaque_is_opaque() {
        let img = image_with_alpha(1.0, 0.0);
        assert_eq!(classify_alpha(Some(&img)), AlphaKind::Opaque);
    }

    #[test]
    fn solid_surface_with_cutout_holes_is_cutout() {
        // Mirrors PAL4 lotus `zjtai3-1`: ~66% opaque, ~31% transparent,
        // ~1% graded edges. Must NOT be Blend — it has to keep depth-write
        // on so the concave bowl self-occludes.
        let img = image_with_alpha(0.665, 0.32);
        assert_eq!(classify_alpha(Some(&img)), AlphaKind::Cutout);
    }

    #[test]
    fn mostly_opaque_soft_edged_surface_is_cutout() {
        // Mirrors PAL4 lotus `zjtai4-4`: ~88% opaque, no fully-transparent
        // texels, a sliver of graded edges.
        let img = image_with_alpha(0.883, 0.0);
        assert_eq!(classify_alpha(Some(&img)), AlphaKind::Cutout);
    }

    #[test]
    fn genuinely_translucent_surface_is_blend() {
        // Mirrors PAL4 water `y02` / cloud `yun02`: ~0% opaque, large
        // graded fraction. Must stay Blend (depth-write off).
        let img = image_with_alpha(0.0, 0.045);
        assert_eq!(classify_alpha(Some(&img)), AlphaKind::Blend);
    }

    #[test]
    fn force_opaque_keeps_rgb_and_classifies_opaque() {
        // A texture that would normally be Blend (graded alpha) and have
        // its RGB premultiplied (darkened). The opaque path must force
        // alpha to 255, classify it Opaque, and leave RGB untouched.
        // Mirrors PAL5 terrain `.dds` (non-coverage alpha).
        let name = "test::force_opaque_terrain";
        let def = TextureStore::get_or_update_opaque(name, || {
            let mut img = RgbaImage::new(64, 64);
            for px in img.pixels_mut() {
                *px = Rgba([200, 180, 120, 42]); // dark-if-premultiplied
            }
            Some(img)
        });
        assert_eq!(def.alpha_kind(), AlphaKind::Opaque);
        let img = def.image.lock().unwrap();
        let img = img.as_ref().expect("image present");
        let px = img.get_pixel(0, 0);
        // RGB preserved (not premultiplied down toward black), alpha forced.
        assert_eq!(px.0, [200, 180, 120, 255]);
    }
}
