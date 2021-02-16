use image::ImageFormat;

use super::{texture::TextureDef, ShaderDef, SIMPLE_SHADER_DEF};
use std::io::Read;

pub trait Material: downcast_rs::Downcast + std::fmt::Debug {}

downcast_rs::impl_downcast!(Material);

pub struct MaterialDef {
    name: String,
    shader: ShaderDef,
    textures: Vec<TextureDef>,
    use_alpha: bool,
}

impl MaterialDef {
    pub fn new(name: &str, shader: ShaderDef, textures: Vec<TextureDef>, use_alpha: bool) -> Self {
        Self {
            name: name.to_string(),
            textures,
            shader,
            use_alpha,
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

    pub fn use_alpha(&self) -> bool {
        self.use_alpha
    }
}

pub struct SimpleMaterialDef;
impl SimpleMaterialDef {
    pub fn create<R: Read>(reader: Option<&mut R>, use_alpha: bool) -> MaterialDef {
        let data = if let Some(r) = reader {
            let mut buf = Vec::new();
            r.read_to_end(&mut buf).unwrap();
            image::load_from_memory(&buf)
                .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                .and_then(|img| Ok(img.to_rgba8()))
                .ok()
        } else {
            None
        };

        MaterialDef::new(
            "simple_material",
            SIMPLE_SHADER_DEF.clone(),
            vec![TextureDef::ImageTextureDef(data)],
            use_alpha,
        )
    }
}
