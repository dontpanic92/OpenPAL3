use std::path::PathBuf;
use super::{Shader, SimpleShader};

pub trait Material {
    fn name(&self) -> &str;
    fn shader(&self) -> &dyn Shader;
}

pub struct SimpleMaterial {
    texture_path: PathBuf,
    shader: SimpleShader,
}

impl SimpleMaterial {
    pub fn new(texture_path: PathBuf) -> Self {
        SimpleMaterial {
            texture_path,
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
}
