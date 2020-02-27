use super::{Shader, SimpleShader};
use crate::rendering::texture::Texture;
use std::path::PathBuf;

pub trait Material {
    fn name(&self) -> &str;
    fn shader(&self) -> &dyn Shader;
    fn textures(&self) -> &Vec<Texture>;
}

pub struct SimpleMaterial {
    textures: Vec<Texture>,
    shader: SimpleShader,
}

impl SimpleMaterial {
    pub fn new(texture_path: &PathBuf) -> Self {
        let texture = Texture::new(texture_path);
        SimpleMaterial {
            textures: vec![texture],
            shader: SimpleShader {},
        }
    }
}

impl Material for SimpleMaterial {
    fn name(&self) -> &str {
        "simple_material"
    }

    fn shader(&self) -> &dyn Shader {
        &self.shader
    }

    fn textures(&self) -> &Vec<Texture> {
        &self.textures
    }
}
