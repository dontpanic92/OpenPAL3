use std::rc::Rc;

use image::RgbaImage;

use crate::rendering::{Material, MaterialDef};

use super::{shader::VitaGLShader, texture::VitaGLTexture};

pub struct VitaGLMaterial {
    name: String,
    shader: Rc<VitaGLShader>,
    textures: Vec<Rc<VitaGLTexture>>,
}

impl Material for VitaGLMaterial {}

impl std::fmt::Debug for VitaGLMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VitaGLMaterial"))
    }
}

impl VitaGLMaterial {
    pub fn new(def: &MaterialDef, shader: Rc<VitaGLShader>) -> Self {
        let textures = def
            .textures()
            .iter()
            .map(|t| {
                let image = t.image().unwrap_or(&TEXTURE_MISSING);
                Rc::new(VitaGLTexture::new(image.width(), image.height(), image))
            })
            .collect();

        Self {
            name: def.name().to_string(),
            shader,
            textures,
        }
    }

    pub fn shader(&self) -> &VitaGLShader {
        &self.shader
    }

    pub fn textures(&self) -> &[Rc<VitaGLTexture>] {
        &self.textures
    }
}

lazy_static::lazy_static! {
static ref TEXTURE_MISSING: RgbaImage =
image::load_from_memory(radiance_assets::TEXTURE_MISSING_TEXTURE_FILE)
    .unwrap()
    .to_rgba8();
}
