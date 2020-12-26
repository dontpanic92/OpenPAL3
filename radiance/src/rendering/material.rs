use image::ImageFormat;

use super::{texture::TextureDef, ShaderDef, SIMPLE_SHADER_DEF};
use std::io::Read;

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

pub struct SimpleMaterialDef;
impl SimpleMaterialDef {
    pub fn create<R: Read>(reader: &mut R) -> MaterialDef {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        let data = image::load_from_memory(&buf)
                .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                .and_then(|img| Ok(img.to_rgba())).ok();

        MaterialDef::new(
            "simple_material",
            SIMPLE_SHADER_DEF.clone(),
            vec![TextureDef::ImageTextureDef(data)],
        )
    }
}
