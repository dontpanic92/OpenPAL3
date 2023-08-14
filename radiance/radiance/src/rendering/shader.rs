use super::VertexComponents;

pub trait Shader: downcast_rs::Downcast {
    fn name(&self) -> &str;
}

downcast_rs::impl_downcast!(Shader);

#[derive(Clone)]
pub struct ShaderDef {
    name: String,
    vertex_components: VertexComponents,
    vert_src: Vec<u8>,
    frag_src: Vec<u8>,
}

cfg_if::cfg_if! {
    if #[cfg(vulkan)] {
        pub static SIMPLE_TRIANGLE_VERT: &'static [u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.vert.spv"));
        pub static SIMPLE_TRIANGLE_FRAG: &'static [u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.frag.spv"));
        pub static LIGHTMAP_TEXTURE_VERT: &'static [u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.vert.spv"));
        pub static LIGHTMAP_TEXTURE_FRAG: &'static [u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.frag.spv"));
    } else {
        pub static SIMPLE_TRIANGLE_VERT: &'static [u8] = &[];
        pub static SIMPLE_TRIANGLE_FRAG: &'static [u8] = &[];
        pub static LIGHTMAP_TEXTURE_VERT: &'static [u8] = &[];
        pub static LIGHTMAP_TEXTURE_FRAG: &'static [u8] = &[];
    }
}

lazy_static! {
    pub static ref SIMPLE_SHADER_DEF: ShaderDef = ShaderDef::new(
        "simple_triangle",
        VertexComponents::POSITION | VertexComponents::TEXCOORD,
        SIMPLE_TRIANGLE_VERT,
        SIMPLE_TRIANGLE_FRAG,
    );
}

impl ShaderDef {
    pub fn new(
        name: &str,
        vertex_components: VertexComponents,
        vert_src: &[u8],
        frag_src: &[u8],
    ) -> Self {
        Self {
            name: name.to_string(),
            vertex_components,
            vert_src: Vec::from(vert_src),
            frag_src: Vec::from(frag_src),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn vertex_components(&self) -> VertexComponents {
        self.vertex_components
    }

    pub fn vert_src(&self) -> &[u8] {
        &self.vert_src
    }

    pub fn frag_src(&self) -> &[u8] {
        &self.frag_src
    }
}

pub struct LightMapShaderDef;
impl LightMapShaderDef {
    pub fn create() -> ShaderDef {
        ShaderDef::new(
            "lightmap_texture",
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2,
            LIGHTMAP_TEXTURE_VERT,
            LIGHTMAP_TEXTURE_FRAG,
        )
    }
}
