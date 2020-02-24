use super::vertex::VertexComponents;

pub trait Shader {
    fn name(&self) -> &str;
    fn vert_src(&self) -> &[u8];
    fn frag_src(&self) -> &[u8];
    fn vertex_components(&self) -> VertexComponents;
}

pub struct SimpleShader {

}

static SIMPLE_TRIANGLE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.vert.spv"));
static SIMPLE_TRIANGLE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.frag.spv"));

impl Shader for SimpleShader {
    fn name(&self) -> &str {
        "simple_triangle"
    }

    fn vertex_components(&self) -> VertexComponents {
        VertexComponents::POSITION | VertexComponents::TEXCOORD
    }

    fn vert_src(&self) -> &[u8] {
        SIMPLE_TRIANGLE_VERT
    }

    fn frag_src(&self) -> &[u8] {
        SIMPLE_TRIANGLE_VERT
    }
}
