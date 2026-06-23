use image::ImageFormat;

use crate::rendering::texture::TextureStore;

use super::{ShaderProgram, sampler::SamplerDef, texture::TextureDef};
use std::{io::Read, sync::Arc};

/// Color-blend mode for a material. Today every variant maps to a distinct
/// Vulkan pipeline; the cross-backend `MaterialDef` only exposes the enum.
///
/// `AlphaTest` is the default and is wired to *reproduce* the legacy
/// behavior of the pipeline (blend always enabled with
/// `SRC_ALPHA / ONE_MINUS_SRC_ALPHA`, plus a `discard` in the fragment
/// shader). Once the GLSL shaders learn about a real opaque vs cutout
/// split, `AlphaTest` will switch to blend-disabled.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlendMode {
    Opaque,
    AlphaTest,
    AlphaBlend,
    Additive,
    Multiply,
}

/// Depth test/write configuration for a material.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DepthMode {
    /// Test enabled, write enabled. Today's default for every material.
    TestWrite,
    /// Test enabled, write disabled. Use for translucent surfaces.
    TestOnly,
    /// Test and write both disabled. Use for overlays / UI / debug.
    Disabled,
}

/// Cull mode for a material.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CullMode {
    Back,
    Front,
    None,
}

/// Default depth mode implied by a blend mode.
///
/// Translucent surfaces (`AlphaBlend`/`Additive`/`Multiply`) must test depth
/// but **not** write it, so overlapping translucent draws never occlude one
/// another through the depth buffer — their visual order is governed purely by
/// the renderer's back-to-front draw order + blending. Opaque and cutout
/// (`AlphaTest`) surfaces keep `TestWrite` so they populate the depth buffer
/// normally. `with_blend` / `blend` apply this automatically; an explicit
/// `with_depth` / `.depth` after the blend call overrides it.
fn depth_for_blend(blend: BlendMode) -> DepthMode {
    match blend {
        BlendMode::Opaque | BlendMode::AlphaTest => DepthMode::TestWrite,
        BlendMode::AlphaBlend | BlendMode::Additive | BlendMode::Multiply => DepthMode::TestOnly,
    }
}

/// Per-material parameters that, once a per-material UBO is wired up, will
/// be uploaded to the fragment shader. For now the values are carried
/// through the material model but consumed only as documentation: the
/// fragment shaders still hardcode the cutoff.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MaterialParams {
    pub tint: [f32; 4],
    pub alpha_ref: f32,
    /// Per-material scalar uploaded to the fragment shader as
    /// `MaterialParamsGpu.misc.y`. Currently consumed only by
    /// `lightmap_texture.frag` as the baked-lighting intensity
    /// (PAL4's `_ltMap.cfg` `intensity` field). `1.0` is the neutral
    /// pass-through value; defaults to `1.0` for all materials so
    /// non-lightmap shaders are unaffected.
    pub intensity: f32,
    /// Baked-lightmap ambient floor, uploaded to the fragment shader as
    /// `MaterialParamsGpu.misc.z` and consumed only by
    /// `lightmap_texture.frag` as the additive term in
    /// `lightMap * 1.5 * intensity + ambient_floor`. Defaults to `0.3`,
    /// which reproduces the previous hard-coded floor (tuned for PAL4's
    /// dark caves) for every existing caller. PAL3, whose baked lightmaps
    /// are the primary lighting and must keep their dark, high-contrast
    /// shadows, sets this to `0.0` so the floor no longer lifts/desaturates
    /// the baked tone. Non-lightmap shaders ignore `misc.z`.
    pub ambient_floor: f32,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            tint: [1.0, 1.0, 1.0, 1.0],
            // Alpha-test punch-through threshold (half coverage). Texels
            // below 50% alpha are discarded; the rest are kept. This is how
            // RenderWare 3.x / Gamebox rendered hair, fur, and foliage —
            // crisp, solid edges that occlude what's behind them.
            //
            // A near-zero threshold (the previous 1/255) instead keeps every
            // faint bilinear-filtered edge texel and alpha-blends it on top
            // of whatever is behind, so soft edges become semi-transparent
            // and the background shows *through* them — e.g. the skybox
            // bleeding through tree-leaf edges, or a character's skin showing
            // through their hair. 0.5 is the classic D3D/RW alpha-ref that
            // avoids that see-through fringe.
            alpha_ref: 0.5,
            intensity: 1.0,
            ambient_floor: 0.3,
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        }
    }
}

/// Identity key used to look up the Vulkan pipeline and the per-material
/// descriptor-set layout. Two materials that produce the same `MaterialKey`
/// share both.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct MaterialKey {
    pub program: ShaderProgram,
    pub blend: BlendMode,
    pub depth: DepthMode,
    pub cull: CullMode,
}

#[derive(Clone)]
pub struct MaterialDef {
    debug_name: String,
    program: ShaderProgram,
    textures: Vec<Arc<TextureDef>>,
    samplers: Vec<SamplerDef>,
    params: MaterialParams,
    blend: BlendMode,
    depth: DepthMode,
    cull: CullMode,
    /// Optional nonce that opts this material out of the renderer's
    /// `MaterialIdentity` cache. When `Some`, two `MaterialDef`s that
    /// are otherwise identical (same textures, same params) still
    /// resolve to *separate* backend `VulkanMaterial` instances.
    ///
    /// Required for materials that get mutated at runtime — e.g. PAL4
    /// water materials whose UV transform is animated by
    /// `UvAnimationComponent`. Without this, the per-material UBO would be
    /// shared with any non-animated material that happens to use the
    /// same texture, leaking the UV scroll onto unrelated geometry like
    /// grass / leaves / hair.
    unique_nonce: Option<u64>,
}

impl MaterialDef {
    /// Construct a `MaterialDef` directly. New code should prefer
    /// [`MaterialDef::builder`].
    pub fn new(name: String, shader: ShaderProgram, textures: Vec<Arc<TextureDef>>) -> Self {
        MaterialDefBuilder::new(shader)
            .debug_name(name)
            .textures(textures)
            .build()
    }

    pub fn builder(program: ShaderProgram) -> MaterialDefBuilder {
        MaterialDefBuilder::new(program)
    }

    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    pub fn shader(&self) -> ShaderProgram {
        self.program
    }

    pub fn program(&self) -> ShaderProgram {
        self.program
    }

    pub fn textures(&self) -> &[Arc<TextureDef>] {
        &self.textures
    }

    /// Per-texture-binding sampler descriptions. Always the same length
    /// as `textures()`. Backends that don't yet route per-material
    /// sampler state are free to ignore this; the default values
    /// (`SamplerDef::DEFAULT` — LINEAR + REPEAT) match today's
    /// hardcoded sampler behavior.
    pub fn samplers(&self) -> &[SamplerDef] {
        &self.samplers
    }

    pub fn params(&self) -> &MaterialParams {
        &self.params
    }

    pub fn blend(&self) -> BlendMode {
        self.blend
    }

    pub fn depth(&self) -> DepthMode {
        self.depth
    }

    pub fn cull(&self) -> CullMode {
        self.cull
    }

    pub fn key(&self) -> MaterialKey {
        MaterialKey {
            program: self.program,
            blend: self.blend,
            depth: self.depth,
            cull: self.cull,
        }
    }

    /// Override the blend mode on an existing `MaterialDef`. Also resets
    /// `params.alpha_ref` to the mode-appropriate default (0.5 for
    /// `AlphaTest` — half-coverage punch-through so faint edge texels are
    /// discarded rather than blended, giving crisp, solid hair/fur/foliage
    /// edges — and 0 for every other mode) so existing call sites that
    /// reach into a `SimpleMaterialDef::create*` result can switch the mode
    /// without thinking about the cutoff.
    pub fn with_blend(mut self, blend: BlendMode) -> Self {
        self.blend = blend;
        self.params.alpha_ref = match blend {
            BlendMode::AlphaTest => 0.5,
            _ => 0.0,
        };
        // Translucent surfaces must test depth but not write it, so they
        // never occlude each other (or geometry behind them) through the
        // depth buffer — their visual order comes purely from the
        // renderer's back-to-front draw order + blending. Without this,
        // an alpha-blended mesh writes depth and a farther translucent
        // mesh drawn afterwards fails the `LESS` test and disappears.
        // Opaque/cutout keep `TestWrite`. Call `with_depth(...)` after
        // `with_blend(...)` to override this default.
        self.depth = depth_for_blend(blend);
        self
    }

    /// Override the depth mode on an existing `MaterialDef`.
    pub fn with_depth(mut self, depth: DepthMode) -> Self {
        self.depth = depth;
        self
    }

    /// Override the cull mode on an existing `MaterialDef`. Used by
    /// loaders that decide two-sidedness after the underlying
    /// `MaterialDef` is built (e.g. RenderWare foliage: leaf/grass
    /// cards are single quads that must render two-sided, otherwise the
    /// back-facing half is culled and the foliage looks sparse or
    /// vanishes entirely). `cull` *is* part of `MaterialIdentity`, so
    /// two materials that differ only in cull mode are cached as
    /// separate backend pipelines automatically.
    pub fn with_cull(mut self, cull: CullMode) -> Self {
        self.cull = cull;
        self
    }

    /// Override the debug name (post-build) — useful when the loader
    /// only learns the source identifier (e.g. an RW material's
    /// `PLUGIN_USERDATA name`) after the underlying `MaterialDef` is
    /// constructed. The name is *not* part of `MaterialIdentity`, so
    /// renaming a material does not affect cache de-duplication;
    /// callers that want a distinct backend material must also call
    /// `make_unique`.
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Override the per-material `MaterialParams` on an existing
    /// `MaterialDef`. Used by loaders that learn the parameters (e.g.
    /// a per-scene `tint`) after the underlying `MaterialDef` is
    /// constructed by a shared builder. Note: `params` *is* part of
    /// `MaterialIdentity`, so two materials whose textures match but
    /// whose params differ are cached as separate backend materials
    /// automatically — no need to also call `make_unique`.
    pub fn with_params(mut self, params: MaterialParams) -> Self {
        self.params = params;
        self
    }

    /// Override the baked-lightmap modulation parameters on an existing
    /// `MaterialDef` (consumed by `lightmap_texture.frag`): `intensity`
    /// scales the baked contribution (`misc.y`) and `ambient_floor` is the
    /// additive term (`misc.z`). PAL3 uses this to set `ambient_floor = 0.0`
    /// so its baked lightmaps keep dark, high-contrast shadows instead of the
    /// PAL4-tuned `0.3` floor that lifts and desaturates them.
    pub fn with_lightmap_params(mut self, intensity: f32, ambient_floor: f32) -> Self {
        self.params.intensity = intensity;
        self.params.ambient_floor = ambient_floor;
        self
    }

    /// Stamp this material with a fresh unique nonce so the renderer
    /// allocates a separate backend `VulkanMaterial` for it rather than
    /// sharing one through the texture/params cache. Use this for
    /// materials that will be mutated at runtime (e.g. PAL4 water
    /// surfaces whose UV affine is updated each frame by
    /// `UvAnimationComponent`). Idempotent — calling it again replaces the
    /// existing nonce with a fresh one.
    pub fn make_unique(mut self) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        self.unique_nonce = Some(COUNTER.fetch_add(1, Ordering::Relaxed));
        self
    }

    /// Nonce previously stamped by [`MaterialDef::make_unique`], if any.
    /// `None` means the material is eligible for cache de-duplication.
    pub fn unique_nonce(&self) -> Option<u64> {
        self.unique_nonce
    }
}

/// Builder for [`MaterialDef`]. Defaults reproduce today's renderer
/// behavior: `BlendMode::AlphaTest`, `DepthMode::TestWrite`,
/// `CullMode::Back`, and default `MaterialParams` (`alpha_ref = 0.5`).
pub struct MaterialDefBuilder {
    debug_name: String,
    program: ShaderProgram,
    textures: Vec<Arc<TextureDef>>,
    samplers: Vec<SamplerDef>,
    params: MaterialParams,
    blend: BlendMode,
    depth: DepthMode,
    cull: CullMode,
}

impl MaterialDefBuilder {
    pub fn new(program: ShaderProgram) -> Self {
        Self {
            debug_name: String::new(),
            program,
            textures: Vec::new(),
            samplers: Vec::new(),
            params: MaterialParams::default(),
            blend: BlendMode::AlphaTest,
            depth: DepthMode::TestWrite,
            cull: CullMode::Back,
        }
    }

    pub fn debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the texture bindings, defaulting their sampler state to
    /// `SamplerDef::DEFAULT` (LINEAR + REPEAT). Use
    /// [`MaterialDefBuilder::textures_with_samplers`] when the loader
    /// has explicit sampler state to forward (e.g. RW
    /// `address_mode_u/v`).
    pub fn textures(mut self, textures: Vec<Arc<TextureDef>>) -> Self {
        self.samplers = vec![SamplerDef::default(); textures.len()];
        self.textures = textures;
        self
    }

    /// Set the texture bindings together with a per-binding
    /// `SamplerDef`. Both vectors must have the same length; this is
    /// asserted at build time.
    pub fn textures_with_samplers(
        mut self,
        textures: Vec<Arc<TextureDef>>,
        samplers: Vec<SamplerDef>,
    ) -> Self {
        assert_eq!(
            textures.len(),
            samplers.len(),
            "textures and samplers must be parallel vectors of equal length",
        );
        self.textures = textures;
        self.samplers = samplers;
        self
    }

    pub fn params(mut self, params: MaterialParams) -> Self {
        self.params = params;
        self
    }

    pub fn blend(mut self, blend: BlendMode) -> Self {
        // Reset `alpha_ref` to the mode-appropriate default. Cutout
        // (`AlphaTest`) uses a half-coverage punch-through threshold so
        // faint edge texels are discarded (crisp, solid edges) rather than
        // alpha-blended into a see-through fringe; every other mode sets it
        // to 0 because the opaque shader variant ignores it. Call
        // `.params(...)` after `.blend(...)` if a custom value is needed.
        self.blend = blend;
        self.params.alpha_ref = match blend {
            BlendMode::AlphaTest => 0.5,
            _ => 0.0,
        };
        // Translucent surfaces test depth but don't write it (see
        // `MaterialDef::with_blend`); opaque/cutout keep `TestWrite`.
        // Call `.depth(...)` after `.blend(...)` to override.
        self.depth = depth_for_blend(blend);
        self
    }

    pub fn depth(mut self, depth: DepthMode) -> Self {
        self.depth = depth;
        self
    }

    pub fn cull(mut self, cull: CullMode) -> Self {
        self.cull = cull;
        self
    }

    pub fn build(self) -> MaterialDef {
        debug_assert_eq!(
            self.textures.len(),
            self.samplers.len(),
            "MaterialDefBuilder: textures/samplers length mismatch (use .textures(...) or .textures_with_samplers(...))",
        );
        MaterialDef {
            debug_name: self.debug_name,
            program: self.program,
            textures: self.textures,
            samplers: self.samplers,
            params: self.params,
            blend: self.blend,
            depth: self.depth,
            cull: self.cull,
            unique_nonce: None,
        }
    }
}

/// `SimpleMaterialDef` and `LightMapMaterialDef` decode their textures via
/// `image::to_rgba8()`. Any texture that carries transparency
/// (`AlphaKind::Cutout` or `AlphaKind::Blend`) is then **premultiplied**
/// in `TextureStore::get_or_update` so the Vulkan blender can use
/// `ONE / ONE_MINUS_SRC_ALPHA` for `BlendMode::AlphaBlend` (and `ONE / ONE`
/// for `Additive`). This avoids the classic black-halo artifact at the
/// edges of bilinear-filtered alpha textures whose RGB was zeroed in
/// fully-transparent texels. Opaque textures (alpha identically 255) and
/// the lightmap channel of `LightMapMaterialDef` skip premultiplication,
/// so opaque draws are bit-identical to before. See
/// `texture::premultiply_alpha` and the comments in
/// `simple_triangle.frag` / `lightmap_texture.frag` for the matching
/// shader-side math.
pub struct SimpleMaterialDef;
impl SimpleMaterialDef {
    pub fn create<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
    ) -> MaterialDef {
        Self::create_with_sampler(texture_name, get_reader, SamplerDef::default())
    }

    pub fn create_with_sampler<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
        sampler: SamplerDef,
    ) -> MaterialDef {
        let texture =
            TextureStore::get_or_update(texture_name, || match get_reader(texture_name) {
                Some(mut r) => {
                    let mut buf = Vec::new();
                    r.read_to_end(&mut buf).unwrap();
                    image::load_from_memory(&buf)
                        .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                        .and_then(|img| Ok(img.to_rgba8()))
                        .ok()
                }
                _ => None,
            });

        Self::create_internal(texture, sampler)
    }

    pub fn create2(texture_name: &str, data: Option<Vec<u8>>) -> MaterialDef {
        Self::create2_with_sampler(texture_name, data, SamplerDef::default())
    }

    /// Like [`SimpleMaterialDef::create2`], but loads the texture through
    /// [`TextureStore::get_or_update_opaque`] so its alpha channel is
    /// ignored (forced opaque) and its RGB is never premultiplied. Use for
    /// textures whose alpha is not coverage data — e.g. PAL5 terrain
    /// `.dds`, which would otherwise render dark.
    pub fn create2_opaque(texture_name: &str, data: Option<Vec<u8>>) -> MaterialDef {
        let texture =
            TextureStore::get_or_update_opaque(texture_name, || decode_texture_data(data));
        Self::create_internal(texture, SamplerDef::default())
    }

    pub fn create2_with_sampler(
        texture_name: &str,
        data: Option<Vec<u8>>,
        sampler: SamplerDef,
    ) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || decode_texture_data(data));

        Self::create_internal(texture, sampler)
    }

    /// Build a `SimpleMaterialDef` directly from an already-decoded
    /// `RgbaImage`. Used by loaders that need to synthesize a composite
    /// texture (e.g. RenderWare DFF materials that pair an RGB texture
    /// with a separate alpha-mask texture) without re-routing through
    /// `image::load_from_memory`. The `texture_name` doubles as the
    /// `TextureStore` cache key, so callers should pick a name that
    /// uniquely identifies the composite (e.g. `"<main>|<mask>"`).
    pub fn create_with_image(texture_name: &str, image: Option<image::RgbaImage>) -> MaterialDef {
        Self::create_with_image_and_sampler(texture_name, image, SamplerDef::default())
    }

    pub fn create_with_image_and_sampler(
        texture_name: &str,
        image: Option<image::RgbaImage>,
        sampler: SamplerDef,
    ) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || image);
        Self::create_internal(texture, sampler)
    }

    fn create_internal(texture_def: Arc<TextureDef>, sampler: SamplerDef) -> MaterialDef {
        MaterialDef::builder(ShaderProgram::TexturedNoLight)
            .debug_name("simple_material")
            .textures_with_samplers(vec![texture_def], vec![sampler])
            .build()
    }
}

/// Decode raw texture bytes (DDS / TGA / PNG / …) into an `RgbaImage`,
/// trying the auto-detected format first and falling back to TGA. Shared
/// by [`SimpleMaterialDef::create2`] and
/// [`SimpleMaterialDef::create2_opaque`].
fn decode_texture_data(data: Option<Vec<u8>>) -> Option<image::RgbaImage> {
    let data = data?;
    image::load_from_memory(&data)
        .or_else(|_| image::load_from_memory_with_format(&data, ImageFormat::Tga))
        .map(|img| img.to_rgba8())
        .ok()
}

/// One terrain-texture layer for [`TerrainSplatMaterialDef`]: a cache
/// `name` plus its raw `.dds`/`.tga` bytes (decoded opaque — terrain
/// texture alpha is non-coverage detail data).
pub struct TerrainLayer {
    pub name: String,
    pub data: Option<Vec<u8>>,
}

/// Builds a PAL5 terrain multi-layer splat material
/// ([`ShaderProgram::TerrainSplat`]). Blends up to four ground textures
/// per-texel by a weight atlas, with dynamic Lambert lighting.
pub struct TerrainSplatMaterialDef;
impl TerrainSplatMaterialDef {
    /// * `debug_name` — material/cache disambiguator (e.g. the block name).
    /// * `layers` — the 4 terrain-texture slots, slot 0 first. Unused slots
    ///   should be padded by the caller (e.g. repeat slot 0) so exactly 4
    ///   are bound.
    /// * `weight_atlas_name` / `weight_atlas` — the per-block weight atlas
    ///   (RGBA = slots 0..3 weights), loaded **raw** so its channels are
    ///   preserved verbatim.
    /// * `active_layers` — number of layers that actually contribute (1..4).
    /// * `tile_world` — world units per full repeat of each ground texture.
    pub fn create(
        debug_name: &str,
        layers: [TerrainLayer; 4],
        weight_atlas_name: &str,
        weight_atlas: image::RgbaImage,
        active_layers: u8,
        tile_world: f32,
    ) -> MaterialDef {
        let mut textures: Vec<Arc<TextureDef>> = layers
            .into_iter()
            .map(|layer| {
                // Terrain ground textures: opaque load (ignore their
                // non-coverage alpha; never premultiply -> no darkening).
                TextureStore::get_or_update_opaque(&layer.name, || decode_texture_data(layer.data))
            })
            .collect();
        // Weight atlas: raw load so R/G/B/A weights survive intact.
        textures.push(TextureStore::get_or_update_raw(weight_atlas_name, || {
            Some(weight_atlas)
        }));

        let samplers = vec![SamplerDef::default(); textures.len()];

        let mut params = MaterialParams::default();
        params.alpha_ref = active_layers as f32; // -> misc.x (layer count)
        let inv_tile = if tile_world != 0.0 {
            1.0 / tile_world
        } else {
            0.0
        };
        params.uv_scale = [inv_tile, inv_tile]; // -> uv_xform.xy (world->tile)

        MaterialDef::builder(ShaderProgram::TerrainSplat)
            .debug_name(debug_name)
            .textures_with_samplers(textures, samplers)
            // `blend()` resets `alpha_ref` (= our `misc.x` layer count) to the
            // mode default, so it MUST come before `params()` or the shader
            // sees a layer count of 0 and never blends the overlay layers
            // (rendering `base * base_weight`, which darkens every
            // overlay-weighted texel toward black).
            .blend(BlendMode::Opaque)
            .cull(CullMode::None)
            .params(params)
            .build()
    }
}

/// Builds a dynamically-lit textured material ([`ShaderProgram::TexturedDynamicLit`]).
/// Same single-texture setup as [`SimpleMaterialDef`], but the fragment shader
/// applies per-pixel Lambert lighting from the scene's lights. Intended for
/// actors whose meshes provide vertex normals.
pub struct LitMaterialDef;
impl LitMaterialDef {
    pub fn create<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
    ) -> MaterialDef {
        Self::create_with_sampler(texture_name, get_reader, SamplerDef::default())
    }

    pub fn create_with_sampler<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
        sampler: SamplerDef,
    ) -> MaterialDef {
        let texture =
            TextureStore::get_or_update(texture_name, || match get_reader(texture_name) {
                Some(mut r) => {
                    let mut buf = Vec::new();
                    r.read_to_end(&mut buf).unwrap();
                    image::load_from_memory(&buf)
                        .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                        .and_then(|img| Ok(img.to_rgba8()))
                        .ok()
                }
                _ => None,
            });

        MaterialDef::builder(ShaderProgram::TexturedDynamicLit)
            .debug_name("lit_material")
            .textures_with_samplers(vec![texture], vec![sampler])
            .build()
    }
}

pub struct LightMapMaterialDef;
impl LightMapMaterialDef {
    pub fn create<R: Read>(
        textures: Vec<&str>,
        get_reader: impl Fn(&str) -> Option<R>,
    ) -> MaterialDef {
        let count = textures.len();
        Self::create_with_samplers(textures, get_reader, vec![SamplerDef::default(); count])
    }

    pub fn create_with_samplers<R: Read>(
        textures: Vec<&str>,
        get_reader: impl Fn(&str) -> Option<R>,
        samplers: Vec<SamplerDef>,
    ) -> MaterialDef {
        assert_eq!(
            textures.len(),
            samplers.len(),
            "LightMapMaterialDef: textures and samplers must be parallel vectors of equal length",
        );
        let textures: Vec<Arc<TextureDef>> = textures
            .into_iter()
            .map(|name| {
                TextureStore::get_or_update(name, || {
                    let mut buf = Vec::new();
                    let b = match get_reader(name) {
                        None => radiance_assets::TEXTURE_WHITE_TEXTURE_FILE,
                        Some(mut reader) => {
                            reader.read_to_end(&mut buf).unwrap();
                            &buf
                        }
                    };

                    image::load_from_memory(b)
                        .or_else(|err| {
                            log::error!("Cannot load texture: {}", &err);
                            Err(err)
                        })
                        .ok()
                        .and_then(|img| Some(img.to_rgba8()))
                })
            })
            .collect();

        MaterialDef::builder(ShaderProgram::TexturedLightmap)
            .debug_name("lightmap_material")
            .textures_with_samplers(textures, samplers)
            .build()
    }
}

/// Per-vertex Y-gradient material. Used by the PAL4 nav-mesh
/// floor/wall debug visualization: every fragment's color is
/// `mix(low, high, t)` where `t = (worldY − y_min) / (y_max − y_min)`.
///
/// Parameters are encoded into the shared [`MaterialParams`] UBO via
/// field-reuse (so the renderer's existing per-material UBO upload
/// path keeps working unchanged):
///
/// | UBO slot         | CPU `MaterialParams` field | Meaning              |
/// |------------------|----------------------------|----------------------|
/// | `tint.rgb`       | `tint[0..3]`               | high color (Y == max)|
/// | `tint.a`         | `tint[3]`                  | `y_max`              |
/// | `misc.x`         | `alpha_ref`                | unused (kept 0)      |
/// | `misc.y`         | `intensity`                | `y_min`              |
/// | `uv_xform.xy`    | `uv_scale`                 | low color R, G       |
/// | `uv_xform.z`     | `uv_offset[0]`             | low color B          |
/// | `uv_xform.w`     | `uv_offset[1]`             | unused (kept 0)      |
///
/// A 1×1 white dummy texture is bound at `texSampler[0]` so the
/// renderer's per-material descriptor layout (keyed on
/// `textures().len()`) doesn't need a zero-texture special case. The
/// fragment shader never samples it.
pub struct GradientYMaterialDef;

impl GradientYMaterialDef {
    /// Build a gradient material with the default blue (low) → red
    /// (high) heatmap colors over `[y_min, y_max]` (world-space Y).
    pub fn create(y_min: f32, y_max: f32) -> MaterialDef {
        Self::create_with_colors(y_min, y_max, [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
    }

    /// Build a gradient material with caller-supplied low/high RGB
    /// colors. `low` is the color at `y_min`, `high` at `y_max`.
    pub fn create_with_colors(
        y_min: f32,
        y_max: f32,
        low: [f32; 3],
        high: [f32; 3],
    ) -> MaterialDef {
        let dummy = TextureStore::get_or_update("__gradient_y_dummy", || {
            // 1×1 white opaque pixel — never sampled, only present so
            // the per-material descriptor layout has textures().len() == 1.
            Some(image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([255, 255, 255, 255]),
            ))
        });

        let params = MaterialParams {
            tint: [high[0], high[1], high[2], y_max],
            alpha_ref: 0.0,
            intensity: y_min,
            ambient_floor: 0.3,
            uv_scale: [low[0], low[1]],
            uv_offset: [low[2], 0.0],
        };

        MaterialDef::builder(ShaderProgram::GradientY)
            .debug_name("gradient_y_material")
            .textures(vec![dummy])
            // Order matters: `blend()` clobbers `alpha_ref`, so set the
            // mode first and then stamp the encoded params.
            .blend(BlendMode::Opaque)
            .params(params)
            .build()
    }
}
