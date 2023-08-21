use std::rc::Rc;

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
                Rc::new(VitaGLTexture::new(
                    t.image().unwrap().width(),
                    t.image().unwrap().height(),
                    t.image().unwrap(),
                ))
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
