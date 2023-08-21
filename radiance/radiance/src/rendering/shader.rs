use super::VertexComponents;

pub trait Shader: downcast_rs::Downcast {
    fn name(&self) -> &str;
}

downcast_rs::impl_downcast!(Shader);

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub enum ShaderProgram {
    TexturedNoLight,
    TexturedLightmap,
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
