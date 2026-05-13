use super::texture::VulkanTextureStore;
use super::{device::Device, shader::VulkanShader, texture::VulkanTexture};
use super::shader_cache::VulkanShaderCache;
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::{Material, MaterialDef, MaterialKey, MaterialParams};
use std::rc::Rc;

pub struct VulkanMaterial {
    debug_name: String,
    key: MaterialKey,
    shader: Rc<VulkanShader>,
    textures: Vec<Rc<VulkanTexture>>,
    params: MaterialParams,
}

impl Material for VulkanMaterial {}

impl std::fmt::Debug for VulkanMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VulkanMaterial: {}", &self.debug_name))
    }
}

impl VulkanMaterial {
    pub fn new(
        def: &MaterialDef,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
        texture_store: &mut VulkanTextureStore,
        shader_cache: &VulkanShaderCache,
    ) -> Self {
        let shader = shader_cache.get_or_create(def.program());
        let textures = def
            .textures()
            .iter()
            .map(|t| {
                texture_store.get_or_update(t.name(), || {
                    VulkanTexture::new(t, device, allocator, command_runner).unwrap()
                })
            })
            .collect();
        Self {
            debug_name: def.debug_name().to_string(),
            key: def.key(),
            shader,
            textures,
            params: *def.params(),
        }
    }

    /// Debug-only name. Do not use as a cache key.
    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Backwards-compatible alias for `debug_name()`.
    pub fn name(&self) -> &str {
        &self.debug_name
    }

    pub fn key(&self) -> &MaterialKey {
        &self.key
    }

    pub fn shader(&self) -> &VulkanShader {
        &self.shader
    }

    pub fn textures(&self) -> &[Rc<VulkanTexture>] {
        &self.textures
    }

    pub fn params(&self) -> &MaterialParams {
        &self.params
    }
}
