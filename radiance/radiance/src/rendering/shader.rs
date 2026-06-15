use super::VertexComponents;

pub trait Shader {
    fn name(&self) -> &str;
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum ShaderProgram {
    TexturedNoLight,
    TexturedLightmap,
    /// Per-vertex Y-gradient. Used by the PAL4 nav-mesh floor/wall
    /// debug visualization: the fragment color is `mix(low, high, t)`
    /// where `t = (worldY - y_min) / (y_max - y_min)`. Parameters are
    /// carried through the shared `MaterialParams` UBO via field-reuse
    /// (see `GradientYMaterialDef`). Texture binding is a single 1×1
    /// white dummy so the existing per-material descriptor-set plumbing
    /// (which expects ≥ 1 texture) keeps working unchanged.
    GradientY,

    /// Dynamically-lit textured shader: per-pixel Lambert diffuse summed
    /// over the scene's omni point lights plus a flat ambient term. Used
    /// by PAL3 actors (MV3 meshes carry per-frame vertex normals). Reads
    /// the lighting environment from the per-frame UBO (set 0); requires
    /// the `NORMAL` vertex component.
    TexturedDynamicLit,
}

pub(crate) struct ShaderProgramData {
    pub(crate) name: &'static str,
    pub(crate) vert_src: &'static [u8],
    pub(crate) frag_src: &'static [u8],
    pub(crate) components: VertexComponents,
}

impl ShaderProgramData {
    pub(crate) fn new(
        name: &'static str,
        vert_src: &'static [u8],
        frag_src: &'static [u8],
        components: VertexComponents,
    ) -> Self {
        Self {
            name,
            vert_src,
            frag_src,
            components,
        }
    }
}
