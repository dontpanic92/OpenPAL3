use image::ImageFormat;

use crate::rendering::texture::TextureStore;

use super::{texture::TextureDef, ShaderProgram};
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

/// Per-material parameters that, once a per-material UBO is wired up, will
/// be uploaded to the fragment shader. For now the values are carried
/// through the material model but consumed only as documentation: the
/// fragment shaders still hardcode the cutoff.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MaterialParams {
    pub tint: [f32; 4],
    pub alpha_ref: f32,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            tint: [1.0, 1.0, 1.0, 1.0],
            // 0.4 matches the literal in simple_triangle.frag /
            // lightmap_texture.frag so default `AlphaTest` materials keep
            // today's cutoff once the shaders consume this value.
            alpha_ref: 0.4,
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
    params: MaterialParams,
    blend: BlendMode,
    depth: DepthMode,
    cull: CullMode,
}

impl MaterialDef {
    /// Construct a `MaterialDef` directly. New code should prefer
    /// [`MaterialDef::builder`].
    pub fn new(
        name: String,
        shader: ShaderProgram,
        textures: Vec<Arc<TextureDef>>,
    ) -> Self {
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
}

/// Builder for [`MaterialDef`]. Defaults reproduce today's renderer
/// behavior: `BlendMode::AlphaTest`, `DepthMode::TestWrite`,
/// `CullMode::Back`, and default `MaterialParams` (`alpha_ref = 0.4`).
pub struct MaterialDefBuilder {
    debug_name: String,
    program: ShaderProgram,
    textures: Vec<Arc<TextureDef>>,
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

    pub fn textures(mut self, textures: Vec<Arc<TextureDef>>) -> Self {
        self.textures = textures;
        self
    }

    pub fn params(mut self, params: MaterialParams) -> Self {
        self.params = params;
        self
    }

    pub fn blend(mut self, blend: BlendMode) -> Self {
        self.blend = blend;
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
        MaterialDef {
            debug_name: self.debug_name,
            program: self.program,
            textures: self.textures,
            params: self.params,
            blend: self.blend,
            depth: self.depth,
            cull: self.cull,
        }
    }
}

pub struct SimpleMaterialDef;
impl SimpleMaterialDef {
    pub fn create<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
    ) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || {
            if let Some(mut r) = get_reader(texture_name) {
                let mut buf = Vec::new();
                r.read_to_end(&mut buf).unwrap();
                image::load_from_memory(&buf)
                    .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                    .and_then(|img| Ok(img.to_rgba8()))
                    .ok()
            } else {
                None
            }
        });

        Self::create_internal(texture)
    }

    pub fn create2(texture_name: &str, data: Option<Vec<u8>>) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || {
            if let Some(data) = data {
                let image = {
                    image::load_from_memory(&data)
                        .or_else(|_| image::load_from_memory_with_format(&data, ImageFormat::Tga))
                };
                image.and_then(|img| Ok(img.to_rgba8())).ok()
            } else {
                None
            }
        });

        Self::create_internal(texture)
    }

    fn create_internal(texture_def: Arc<TextureDef>) -> MaterialDef {
        MaterialDef::builder(ShaderProgram::TexturedNoLight)
            .debug_name("simple_material")
            .textures(vec![texture_def])
            .build()
    }
}

pub struct LightMapMaterialDef;
impl LightMapMaterialDef {
    pub fn create<R: Read>(
        textures: Vec<&str>,
        get_reader: impl Fn(&str) -> Option<R>,
    ) -> MaterialDef {
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
            .textures(textures)
            .build()
    }
}
