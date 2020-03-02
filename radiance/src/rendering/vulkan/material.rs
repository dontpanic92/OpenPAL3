use super::{
    image_view::ImageView, sampler::Sampler, shader::VulkanShader,
    VulkanRenderingEngine, texture::VulkanTexture,
};
use crate::rendering::Material;
use ash::vk;
use std::error::Error;

pub struct VulkanMaterial {
    shader: VulkanShader,
    name: String,
    textures: Vec<VulkanTexture>,
}

impl VulkanMaterial {
    pub fn new(
        material: &dyn Material,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let device = engine.device();
        let shader = VulkanShader::new(device, material.shader())?;
        let textures = material.textures().iter().map(|t| VulkanTexture::new(t, engine).unwrap()).collect();

        Ok(Self {
            shader,
            name: material.name().to_owned(),
            textures,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shader(&self) -> &VulkanShader {
        &self.shader
    }

    pub fn textures(&self) -> &[VulkanTexture] {
        &self.textures
    }
}
