use crate::rendering::Material;

pub struct VitaGLMaterial {}

impl Material for VitaGLMaterial {}

impl std::fmt::Debug for VitaGLMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VitaGLMaterial"))
    }
}

impl VitaGLMaterial {
    pub fn new() -> Self {
        Self {}
    }
}
