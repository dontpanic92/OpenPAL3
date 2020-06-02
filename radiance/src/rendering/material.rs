use super::{texture::TextureDef, ShaderDef};

pub trait Material: downcast_rs::Downcast + std::fmt::Debug {}

downcast_rs::impl_downcast!(Material);

pub struct MaterialDef {
    name: String,
    shader: ShaderDef,
    textures: Vec<TextureDef>,
}

impl MaterialDef {
    pub fn new(name: &str, shader: ShaderDef, textures: Vec<TextureDef>) -> Self {
        Self {
            name: name.to_string(),
            textures,
            shader,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shader(&self) -> &ShaderDef {
        &self.shader
    }

    pub fn textures(&self) -> &[TextureDef] {
        &self.textures
    }
}
