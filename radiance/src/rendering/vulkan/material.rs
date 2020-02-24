
use super::shader::VulkanShader;

pub struct VulkanMaterial {
    shader: VulkanShader,
}

impl VulkanMaterial {
    pub fn new(shader: VulkanShader) -> Self {
        Self {
            shader,
        }
    }
    
    pub fn shader(&self) -> &VulkanShader {
        &self.shader
    }
}
