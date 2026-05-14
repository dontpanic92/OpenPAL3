use super::buffer::{Buffer, BufferType};
use super::descriptor_managers::DescriptorManager;
use super::sampler::Sampler;
use super::shader_cache::VulkanShaderCache;
use super::texture::VulkanTextureStore;
use super::uniform_buffers::MaterialParamsGpu;
use super::{device::Device, shader::VulkanShader, texture::VulkanTexture};
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::{MaterialDef, MaterialKey, MaterialParams};
use ash::vk;
use std::rc::Rc;

pub struct VulkanMaterial {
    debug_name: String,
    key: MaterialKey,
    shader: Rc<VulkanShader>,
    textures: Vec<Rc<VulkanTexture>>,
    samplers: Vec<Rc<Sampler>>,
    params: MaterialParams,

    descriptor_manager: Rc<DescriptorManager>,
    _params_buffer: Buffer,
    params_descriptor_set: vk::DescriptorSet,
}

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
        descriptor_manager: &Rc<DescriptorManager>,
    ) -> Self {
        let _ = device;
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
        let samplers = def
            .samplers()
            .iter()
            .map(|s| descriptor_manager.sampler_cache().get_or_create(s))
            .collect();

        let gpu_params = MaterialParamsGpu::from_params(def.params());
        let params_buffer =
            Buffer::new_dynamic_buffer_with_data(allocator, BufferType::Uniform, &[gpu_params])
                .expect("failed to allocate material params UBO");
        let params_descriptor_set = descriptor_manager
            .allocate_material_params_descriptor_set(&params_buffer)
            .expect("failed to allocate material params descriptor set");

        Self {
            debug_name: def.debug_name().to_string(),
            key: def.key(),
            shader,
            textures,
            samplers,
            params: *def.params(),
            descriptor_manager: descriptor_manager.clone(),
            _params_buffer: params_buffer,
            params_descriptor_set,
        }
    }

    pub fn debug_name(&self) -> &str {
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

    pub fn samplers(&self) -> &[Rc<Sampler>] {
        &self.samplers
    }

    pub fn params(&self) -> &MaterialParams {
        &self.params
    }

    pub fn material_params_descriptor_set(&self) -> vk::DescriptorSet {
        self.params_descriptor_set
    }
}

impl Drop for VulkanMaterial {
    fn drop(&mut self) {
        self.descriptor_manager
            .free_material_params_descriptor_set(self.params_descriptor_set);
    }
}
