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
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct VulkanMaterial {
    debug_name: String,
    key: MaterialKey,
    shader: Rc<VulkanShader>,
    textures: Vec<Rc<VulkanTexture>>,
    texture_names: Vec<String>,
    samplers: Vec<Rc<Sampler>>,
    /// Current per-material parameters. The `tint` / `misc` fields are
    /// frozen at construction; `uv_scale` / `uv_offset` may be updated at
    /// runtime via [`update_uv_xform`] to drive UV animation (PAL4
    /// water).
    params: Cell<MaterialParams>,

    descriptor_manager: Rc<DescriptorManager>,
    params_buffer: RefCell<Buffer>,
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
        let texture_names: Vec<String> = def
            .textures()
            .iter()
            .map(|t| t.name().to_string())
            .collect();
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
            texture_names,
            samplers,
            params: Cell::new(*def.params()),
            descriptor_manager: descriptor_manager.clone(),
            params_buffer: RefCell::new(params_buffer),
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

    /// Per-binding texture names captured from the source `MaterialDef`
    /// at construction time. Exposed so callers (e.g. the PAL4
    /// `UvAnimDriver` inventory dump) can answer "what texture does
    /// material X use?" without needing the original `MaterialDef`.
    pub fn texture_names(&self) -> &[String] {
        &self.texture_names
    }

    pub fn samplers(&self) -> &[Rc<Sampler>] {
        &self.samplers
    }

    pub fn params(&self) -> MaterialParams {
        self.params.get()
    }

    pub fn material_params_descriptor_set(&self) -> vk::DescriptorSet {
        self.params_descriptor_set
    }

    /// Re-upload `MaterialParamsGpu` with a new UV affine
    /// (`scale = xy`, `offset = zw`). Used by the runtime
    /// `UvAnimComponent` to drive PAL4 water animation each tick.
    ///
    /// `tint` and `misc` are preserved from the immutable `MaterialDef`
    /// captured at construction; only the `uv_xform` slot is rewritten.
    /// Uses interior mutability so the existing `Rc<VulkanMaterial>`
    /// sharing pattern keeps working.
    pub fn update_uv_xform(&self, scale: [f32; 2], offset: [f32; 2]) {
        let mut p = self.params.get();
        p.uv_scale = scale;
        p.uv_offset = offset;
        self.params.set(p);
        let gpu = MaterialParamsGpu::from_params(&p);
        self.params_buffer.borrow_mut().copy_memory_from(&[gpu]);
    }
}

impl Drop for VulkanMaterial {
    fn drop(&mut self) {
        self.descriptor_manager
            .free_material_params_descriptor_set(self.params_descriptor_set);
    }
}
