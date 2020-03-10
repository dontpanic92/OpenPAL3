use super::{shader::VulkanShader, texture::VulkanTexture};
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::Material;
use ash::Device;
use std::error::Error;
use std::rc::Rc;

pub struct VulkanMaterial {
    shader: VulkanShader,
    name: String,
    textures: Vec<VulkanTexture>,
}

impl VulkanMaterial {
    pub fn new(
        material: &dyn Material,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
    ) -> Result<Self, Box<dyn Error>> {
        let shader = VulkanShader::new(device, material.shader())?;
        let textures = material
            .textures()
            .iter()
            .map(|t| VulkanTexture::new(t, device, allocator, command_runner).unwrap())
            .collect();

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
