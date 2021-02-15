use super::{device::Device, shader::VulkanShader, texture::VulkanTexture};
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::{Material, MaterialDef};
use std::rc::Rc;

pub struct VulkanMaterial {
    name: String,
    shader: VulkanShader,
    textures: Vec<VulkanTexture>,
    use_alpha: bool,
}

impl Material for VulkanMaterial {}

impl std::fmt::Debug for VulkanMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VulkanMaterial: {}", &self.name))
    }
}

impl VulkanMaterial {
    pub fn new(
        def: &MaterialDef,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
    ) -> Self {
        let shader = VulkanShader::new(def.shader(), device.clone()).unwrap();
        let textures = def
            .textures()
            .iter()
            .map(|t| VulkanTexture::new(t, device, allocator, command_runner).unwrap())
            .collect();
        Self {
            name: def.name().to_string(),
            shader,
            textures,
            use_alpha: def.use_alpha(),
        }
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

    pub fn use_alpha(&self) -> bool {
        self.use_alpha
    }
}
