use super::{
    adhoc_command_runner::AdhocCommandRunner, buffer::Buffer, descriptor_pool::DescriptorPool,
    descriptor_pool::DescriptorPoolCreateInfo, descriptor_set_layout::DescriptorSetLayout,
    device::Device, image::Image, image_view::ImageView, instance::Instance,
    sampler::VulkanSamplerCache, texture::VulkanTexture,
};
use crate::rendering::ShaderProgram;
use crate::rendering::vulkan::material::VulkanMaterial;
use crate::rendering::vulkan::uniform_buffers::{MaterialParamsGpu, PerFrameUniformBuffer};
use ash::prelude::VkResult;
use ash::vk;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

const MAX_DESCRIPTOR_SET_COUNT: u32 = 40960;
const MAX_DESCRIPTOR_COUNT: u32 = 40960;

type PerMaterialLayoutKey = (ShaderProgram, u32);

pub struct DescriptorManager {
    device: Rc<Device>,
    texture_pool: vk::DescriptorPool,
    per_frame_pool: vk::DescriptorPool,
    per_object_pool: vk::DescriptorPool,
    per_material_params_pool: vk::DescriptorPool,
    texture_layout: vk::DescriptorSetLayout,
    per_frame_layout: vk::DescriptorSetLayout,
    per_material_params_layout: vk::DescriptorSetLayout,
    per_material_layouts: Arc<Mutex<HashMap<PerMaterialLayoutKey, vk::DescriptorSetLayout>>>,
    dub_descriptor_manager: DynamicUniformBufferDescriptorManager,
    sampler_cache: VulkanSamplerCache,

    /// Placeholder 1×1 depth image written into per-frame **binding 1**
    /// (the shadow map sampler) for every set by default. The swapchain
    /// overwrites that binding with the real shadow map; render targets and
    /// any other per-frame set that never runs a shadow pass keep this dummy
    /// so the descriptor is always valid (shadows are disabled there via
    /// `shadow_params.x = 0`, so it is never actually sampled).
    _dummy_shadow_image: Image,
    _dummy_shadow_view: ImageView,
    dummy_shadow_sampler: vk::Sampler,
}

impl DescriptorManager {
    pub fn new(
        device: Rc<Device>,
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &AdhocCommandRunner,
    ) -> VkResult<Self> {
        let texture_pool = Self::create_texture_descriptor_pool(&device).unwrap();
        let per_frame_pool = Self::create_per_frame_descriptor_pool(&device).unwrap();
        let per_object_pool = Self::create_per_object_descriptor_pool(&device).unwrap();
        let per_material_params_pool =
            Self::create_per_material_params_descriptor_pool(&device).unwrap();
        let texture_layout = Self::create_descriptor_set_layout(
            &device,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            vk::ShaderStageFlags::FRAGMENT,
            1,
        )?;
        // Per-frame set 0:
        //   binding 0 — per-frame UBO (view/proj + scene lighting + shadow
        //     matrix). Read by VERTEX (view/proj/lightViewProj) and FRAGMENT
        //     (lighting + shadow sampling), so visible to both stages.
        //   binding 1 — directional shadow map (COMBINED_IMAGE_SAMPLER),
        //     sampled only by the lit FRAGMENT shaders.
        let per_frame_layout = Self::create_per_frame_descriptor_set_layout(&device)?;
        let per_material_params_layout = Self::create_descriptor_set_layout(
            &device,
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            1,
        )?;
        let dub_descriptor_manager = DynamicUniformBufferDescriptorManager::new(device.clone());
        let sampler_cache = VulkanSamplerCache::new(device.clone());

        // Dummy shadow map: a 1×1 depth **array** (matching the lit shaders'
        // `sampler2DArray`) transitioned to a sampleable read-only layout, plus
        // a clamp-to-white-border sampler. Off-map / unsampled lookups read
        // `1.0` ("lit"). The real per-cascade array overwrites binding 1 in the
        // swapchain path.
        let mut dummy_shadow_image = Image::new_depth_array_sampled_image(
            &instance.vk_instance(),
            physical_device,
            allocator,
            1,
            1,
            super::shadow_map::CASCADE_COUNT as u32,
        )
        .expect("failed to allocate dummy shadow image");
        dummy_shadow_image
            .transit_layout(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                command_runner,
            )
            .expect("failed to transition dummy shadow image");
        let dummy_shadow_view = ImageView::new_depth_array_view(
            device.clone(),
            dummy_shadow_image.vk_image(),
            dummy_shadow_image.vk_format(),
            super::shadow_map::CASCADE_COUNT as u32,
        )
        .expect("failed to create dummy shadow image view");
        let dummy_shadow_sampler = create_shadow_sampler(&device)?;

        Ok(Self {
            device,
            texture_pool,
            per_frame_pool,
            per_object_pool,
            per_material_params_pool,
            texture_layout,
            per_frame_layout,
            per_material_params_layout,
            per_material_layouts: Arc::new(Mutex::new(HashMap::new())),
            dub_descriptor_manager,
            sampler_cache,
            _dummy_shadow_image: dummy_shadow_image,
            _dummy_shadow_view: dummy_shadow_view,
            dummy_shadow_sampler,
        })
    }

    /// Layout of the per-frame descriptor set (set 0). Exposed so the
    /// directional shadow depth pipeline can be built against the same set-0
    /// layout the scene pipelines use (Vulkan pipeline-layout compatibility
    /// for set 0).
    pub fn per_frame_layout(&self) -> vk::DescriptorSetLayout {
        self.per_frame_layout
    }

    /// Layout of the per-instance dynamic UBO (set 1: the model matrix).
    /// Needed alongside [`Self::per_frame_layout`] to assemble the shadow
    /// depth pipeline layout.
    pub fn dub_layout(&self) -> vk::DescriptorSetLayout {
        self.dub_descriptor_manager.layout().vk_layout()
    }

    pub fn dub_descriptor_manager(&self) -> &DynamicUniformBufferDescriptorManager {
        &self.dub_descriptor_manager
    }

    pub fn sampler_cache(&self) -> &VulkanSamplerCache {
        &self.sampler_cache
    }

    pub fn create_texture_descriptor_set(&self, texture: &VulkanTexture) -> vk::DescriptorSet {
        let sampler = self.sampler_cache.default_sampler();
        let set = self.allocate_texture_descriptor_set().unwrap();
        let image_info = [vk::DescriptorImageInfo {
            sampler: sampler.vk_sampler(),
            image_view: texture.image_view().vk_image_view(),
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];

        let writes = [vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info)];
        self.device.update_descriptor_sets(&writes, &[]);

        set
    }

    pub fn allocate_texture_descriptor_set(&self) -> VkResult<vk::DescriptorSet> {
        let set = {
            let layouts = [self.texture_layout];
            let allocate_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(self.texture_pool)
                .set_layouts(&layouts);

            self.device.allocate_descriptor_sets(&allocate_info)?[0]
        };

        Ok(set)
    }

    pub fn free_texture_descriptor_set(&self, descriptor_set: vk::DescriptorSet) {
        self.device
            .free_descriptor_sets(self.texture_pool, &[descriptor_set]);
    }

    /// Allocates a `COMBINED_IMAGE_SAMPLER` descriptor set bound to the
    /// given `image_view` with the default sampler. Used by render targets
    /// to register their color image with `imgui-rs-vulkan-renderer`.
    pub fn create_image_view_descriptor_set(&self, image_view: vk::ImageView) -> vk::DescriptorSet {
        let sampler = self.sampler_cache.default_sampler();
        let set = self.allocate_texture_descriptor_set().unwrap();
        let image_info = [vk::DescriptorImageInfo {
            sampler: sampler.vk_sampler(),
            image_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];

        let writes = [vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info)];
        self.device.update_descriptor_sets(&writes, &[]);

        set
    }

    pub fn allocate_per_object_descriptor_set(
        &self,
        material: &VulkanMaterial,
    ) -> VkResult<vk::DescriptorSet> {
        let layout = self.get_per_material_descriptor_layout(material);
        let layouts = [layout];
        let create_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.per_object_pool)
            .set_layouts(&layouts);
        let descriptor_sets = self.device.allocate_descriptor_sets(&create_info)?;

        let image_info: Vec<vk::DescriptorImageInfo> = material
            .textures()
            .iter()
            .zip(material.samplers().iter())
            .map(|(t, s)| {
                vk::DescriptorImageInfo::default()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(t.image_view().vk_image_view())
                    .sampler(s.vk_sampler())
            })
            .collect();

        let write_descriptor_set = vk::WriteDescriptorSet::default()
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .dst_set(descriptor_sets[0])
            .dst_binding(0)
            .dst_array_element(0)
            .image_info(&image_info);
        let write_descriptor_sets = [write_descriptor_set];
        self.device
            .update_descriptor_sets(&write_descriptor_sets, &[]);

        Ok(descriptor_sets[0])
    }

    pub fn free_per_object_descriptor_set(&self, descriptor_set: vk::DescriptorSet) {
        self.device
            .free_descriptor_sets(self.per_object_pool, &[descriptor_set]);
    }

    pub fn allocate_per_frame_descriptor_sets(
        &self,
        uniform_buffers: &[Buffer],
    ) -> VkResult<Vec<vk::DescriptorSet>> {
        let layouts = vec![self.per_frame_layout; uniform_buffers.len()];
        let create_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.per_frame_pool)
            .set_layouts(layouts.as_slice());
        let descriptor_sets = self.device.allocate_descriptor_sets(&create_info)?;

        for i in 0..uniform_buffers.len() {
            let uniform_buffer_info = vk::DescriptorBufferInfo::default()
                .buffer(uniform_buffers[i].vk_buffer())
                .offset(0)
                .range(std::mem::size_of::<PerFrameUniformBuffer>() as u64);

            let buffer_info_array = [uniform_buffer_info];
            // binding 1 (shadow map) defaults to the dummy view+sampler so the
            // set is fully valid even when no shadow pass writes it. Callers
            // that run a shadow pass (the swapchain) overwrite binding 1 via
            // `write_shadow_map_binding`.
            let shadow_image_info = [vk::DescriptorImageInfo::default()
                .sampler(self.dummy_shadow_sampler)
                .image_view(self._dummy_shadow_view.vk_image_view())
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
            let write_descriptor_sets = [
                vk::WriteDescriptorSet::default()
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .dst_set(descriptor_sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .buffer_info(&buffer_info_array),
                vk::WriteDescriptorSet::default()
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .dst_set(descriptor_sets[i])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .image_info(&shadow_image_info),
            ];
            self.device
                .update_descriptor_sets(&write_descriptor_sets, &[])
        }

        Ok(descriptor_sets)
    }

    /// Overwrite **binding 1** (the shadow map sampler) of the supplied
    /// per-frame sets with the real directional shadow map (`image_view` in
    /// `DEPTH_STENCIL_READ_ONLY_OPTIMAL`, `sampler` clamp-to-white-border).
    /// Called by the swapchain after allocating its per-frame sets.
    pub fn write_shadow_map_binding(
        &self,
        descriptor_sets: &[vk::DescriptorSet],
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) {
        for set in descriptor_sets {
            let image_info = [vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(image_view)
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
            let writes = [vk::WriteDescriptorSet::default()
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .dst_set(*set)
                .dst_binding(1)
                .dst_array_element(0)
                .image_info(&image_info)];
            self.device.update_descriptor_sets(&writes, &[]);
        }
    }

    pub fn free_per_frame_descriptor_sets(&self, descriptor_sets: &[vk::DescriptorSet]) {
        self.device
            .free_descriptor_sets(self.per_frame_pool, descriptor_sets);
    }

    pub fn reset_per_frame_descriptor_pool(&self) {
        self.device
            .reset_descriptor_pool(self.per_frame_pool)
            .unwrap();
    }

    pub fn get_vk_descriptor_set_layouts(
        &self,
        material: &VulkanMaterial,
    ) -> [vk::DescriptorSetLayout; 4] {
        let per_material_layout = self.get_per_material_descriptor_layout(material);
        [
            self.per_frame_layout,
            self.dub_descriptor_manager.layout().vk_layout(),
            per_material_layout,
            self.per_material_params_layout,
        ]
    }

    /// Allocate a descriptor set bound to a 1-binding UBO (set = 3,
    /// binding = 0) carrying the material's [`MaterialParamsGpu`].
    pub fn allocate_material_params_descriptor_set(
        &self,
        buffer: &Buffer,
    ) -> VkResult<vk::DescriptorSet> {
        let layouts = [self.per_material_params_layout];
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.per_material_params_pool)
            .set_layouts(&layouts);
        let set = self.device.allocate_descriptor_sets(&allocate_info)?[0];

        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(buffer.vk_buffer())
            .offset(0)
            .range(std::mem::size_of::<MaterialParamsGpu>() as u64)];
        let writes = [vk::WriteDescriptorSet::default()
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .dst_set(set)
            .dst_binding(0)
            .dst_array_element(0)
            .buffer_info(&buffer_info)];
        self.device.update_descriptor_sets(&writes, &[]);

        Ok(set)
    }

    pub fn free_material_params_descriptor_set(&self, descriptor_set: vk::DescriptorSet) {
        self.device
            .free_descriptor_sets(self.per_material_params_pool, &[descriptor_set]);
    }

    fn create_texture_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let sampler_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER);

        let pool_sizes = [sampler_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT);

        device.create_descriptor_pool(&create_info)
    }

    fn create_per_frame_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        // Each per-frame set holds one UBO (binding 0) + one combined image
        // sampler (binding 1, the shadow map). Size both descriptor classes
        // generously — render targets also draw from this pool.
        let uniform_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::UNIFORM_BUFFER);
        let sampler_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER);

        let pool_sizes = [uniform_pool_size, sampler_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT);

        device.create_descriptor_pool(&create_info)
    }

    fn create_per_object_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let sampler_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER);

        let pool_sizes = [sampler_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT);

        device.create_descriptor_pool(&create_info)
    }

    fn create_per_material_params_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let uniform_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::UNIFORM_BUFFER);

        let pool_sizes = [uniform_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT);

        device.create_descriptor_pool(&create_info)
    }

    fn create_descriptor_set_layout(
        device: &Device,
        descriptor_type: vk::DescriptorType,
        stage_flags: vk::ShaderStageFlags,
        descriptor_count: u32,
    ) -> VkResult<vk::DescriptorSetLayout> {
        let layout_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_count(descriptor_count)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags);

        let bindings = [layout_binding];
        let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        device.create_descriptor_set_layout(&create_info)
    }

    /// Two-binding set-0 layout: the per-frame UBO (binding 0, VERTEX +
    /// FRAGMENT) and the directional shadow map sampler (binding 1, FRAGMENT).
    /// Binding 0 must reach FRAGMENT so the lit shaders can read the lighting
    /// + shadow matrix; binding 1 is sampled only by FRAGMENT.
    fn create_per_frame_descriptor_set_layout(
        device: &Device,
    ) -> VkResult<vk::DescriptorSetLayout> {
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        device.create_descriptor_set_layout(&create_info)
    }

    fn get_per_material_descriptor_layout(
        &self,
        material: &VulkanMaterial,
    ) -> vk::DescriptorSetLayout {
        let key: PerMaterialLayoutKey = (material.key().program, material.textures().len() as u32);
        let mut per_material_layouts = self.per_material_layouts.lock().unwrap();
        if !per_material_layouts.contains_key(&key) {
            let layout = Self::create_descriptor_set_layout(
                &self.device,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
                key.1,
            )
            .unwrap();
            per_material_layouts.insert(key, layout);
        }

        *per_material_layouts.get(&key).unwrap()
    }
}

impl Drop for DescriptorManager {
    fn drop(&mut self) {
        self.device.destroy_sampler(self.dummy_shadow_sampler);
        self.device
            .destroy_descriptor_set_layout(self.texture_layout);
        self.device
            .destroy_descriptor_set_layout(self.per_frame_layout);
        self.device
            .destroy_descriptor_set_layout(self.per_material_params_layout);

        for layout in self.per_material_layouts.lock().unwrap().values() {
            self.device.destroy_descriptor_set_layout(*layout);
        }

        self.device.destroy_descriptor_pool(self.texture_pool);
        self.device.destroy_descriptor_pool(self.per_frame_pool);
        self.device.destroy_descriptor_pool(self.per_object_pool);
        self.device
            .destroy_descriptor_pool(self.per_material_params_pool);
    }
}

/// Build the sampler used for the directional shadow map: linear filtering,
/// `CLAMP_TO_BORDER` with an opaque-**white** border so off-map projected
/// lookups read depth `1.0` ("fully lit"). PCF is done manually in-shader, so
/// depth comparison is left disabled.
pub(super) fn create_shadow_sampler(device: &Rc<Device>) -> VkResult<vk::Sampler> {
    let info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
        .anisotropy_enable(false)
        .max_anisotropy(1.0)
        .unnormalized_coordinates(false)
        .compare_enable(false)
        .compare_op(vk::CompareOp::ALWAYS)
        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
        .mip_lod_bias(0.0)
        .min_lod(0.0)
        .max_lod(0.0);
    device.create_sampler(&info)
}

pub struct DynamicUniformBufferDescriptorManager {
    device: Rc<Device>,
    pool: DescriptorPool,
    layout: DescriptorSetLayout,
}

impl DynamicUniformBufferDescriptorManager {
    pub fn new(device: Rc<Device>) -> Self {
        let pool = DescriptorPool::new(
            device.clone(),
            &[DescriptorPoolCreateInfo {
                ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                descriptor_count: 1,
            }],
        );
        let layout = DescriptorSetLayout::new(
            device.clone(),
            vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
            vk::ShaderStageFlags::VERTEX,
            1,
        );

        Self {
            device,
            pool,
            layout,
        }
    }

    pub fn pool(&self) -> &DescriptorPool {
        &self.pool
    }

    pub fn layout(&self) -> &DescriptorSetLayout {
        &self.layout
    }

    pub fn allocate_descriptor_sets(&self, dynamic_uniform_buffer: &Buffer) -> vk::DescriptorSet {
        let layouts = [self.layout.vk_layout()];
        let create_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool.vk_pool())
            .set_layouts(&layouts);
        let descriptor_sets = self.device.allocate_descriptor_sets(&create_info).unwrap();

        let buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(dynamic_uniform_buffer.vk_buffer())
            .offset(0)
            .range(dynamic_uniform_buffer.element_size() as u64);

        let buffer_info_array = [buffer_info];
        let write_descriptor_set = vk::WriteDescriptorSet::default()
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .dst_set(descriptor_sets[0])
            .dst_binding(0)
            .dst_array_element(0)
            .buffer_info(&buffer_info_array);
        let write_descriptor_sets = [write_descriptor_set];
        self.device
            .update_descriptor_sets(&write_descriptor_sets, &[]);

        descriptor_sets[0]
    }
}
