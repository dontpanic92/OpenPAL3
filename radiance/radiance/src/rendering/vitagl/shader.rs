use crate::rendering::Shader;

pub struct VitaGLShader {}

impl Shader for VitaGLShader {
    fn name(&self) -> &str {
        "test"
    }
}

impl VitaGLShader {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
