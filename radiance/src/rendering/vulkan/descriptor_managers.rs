use super::{
    buffer::Buffer, descriptor_pool::DescriptorPool, descriptor_pool::DescriptorPoolCreateInfo,
    descriptor_set_layout::DescriptorSetLayout,
};
use crate::rendering::vulkan::material::VulkanMaterial;
use crate::rendering::vulkan::uniform_buffers::PerFrameUniformBuffer;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::Mutex;

const MAX_DESCRIPTOR_SET_COUNT: u32 = 40960;
const MAX_DESCRIPTOR_COUNT: u32 = 40960;
const MAX_SWAPCHAIN_IMAGE_COUNT: u32 = 4;

pub struct DescriptorManager {
    device: Weak<Device>,
    per_frame_pool: vk::DescriptorPool,
    per_object_pool: vk::DescriptorPool,
    per_frame_layout: vk::DescriptorSetLayout,
    per_material_layouts: Arc<Mutex<HashMap<String, vk::DescriptorSetLayout>>>,
    dub_descriptor_manager: DynamicUniformBufferDescriptorManager,
}

impl DescriptorManager {
    pub fn new(device: &Rc<Device>) -> VkResult<Self> {
        let per_frame_pool = Self::create_per_frame_descriptor_pool(device).unwrap();
        let per_object_pool = Self::create_per_object_descriptor_pool(device).unwrap();
        let per_frame_layout = Self::create_descriptor_set_layout(
            device,
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::ShaderStageFlags::VERTEX,
            1,
        )?;
        let dub_descriptor_manager = DynamicUniformBufferDescriptorManager::new(device);

        Ok(Self {
            device: Rc::downgrade(device),
            per_frame_pool,
            per_object_pool,
            per_frame_layout,
            per_material_layouts: Arc::new(Mutex::new(HashMap::new())),
            dub_descriptor_manager,
        })
    }

    pub fn dub_descriptor_manager(&self) -> &DynamicUniformBufferDescriptorManager {
        &self.dub_descriptor_manager
    }

    pub fn allocate_per_object_descriptor_set(
        &self,
        material: &VulkanMaterial,
    ) -> VkResult<vk::DescriptorSet> {
        let device = self.device.upgrade().unwrap();
        let layout = self.get_per_material_descriptor_layout(material);
        let layouts = [layout];
        let create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.per_object_pool)
            .set_layouts(&layouts)
            .build();
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&create_info) }?;

        let image_info: Vec<vk::DescriptorImageInfo> = material
            .textures()
            .iter()
            .map(|t| {
                vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(t.image_view().vk_image_view())
                    .sampler(t.sampler().vk_sampler())
                    .build()
            })
            .collect();

        let write_descriptor_set = vk::WriteDescriptorSet::builder()
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .dst_set(descriptor_sets[0])
            .dst_binding(0)
            .dst_array_element(0)
            .image_info(&image_info)
            .build();
        let write_descriptor_sets = [write_descriptor_set];
        unsafe { device.update_descriptor_sets(&write_descriptor_sets, &[]) };

        Ok(descriptor_sets[0])
    }

    pub fn allocate_per_frame_descriptor_sets(
        &self,
        uniform_buffers: &[Buffer],
    ) -> VkResult<Vec<vk::DescriptorSet>> {
        let device = self.device.upgrade().unwrap();
        let layouts = vec![self.per_frame_layout; uniform_buffers.len()];
        let create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.per_frame_pool)
            .set_layouts(layouts.as_slice())
            .build();
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&create_info) }?;

        for i in 0..uniform_buffers.len() {
            let uniform_buffer_info = vk::DescriptorBufferInfo::builder()
                .buffer(uniform_buffers[i].vk_buffer())
                .offset(0)
                .range(std::mem::size_of::<PerFrameUniformBuffer>() as u64)
                .build();

            let buffer_info_array = [uniform_buffer_info];
            let write_descriptor_set = vk::WriteDescriptorSet::builder()
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .dst_set(descriptor_sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .buffer_info(&buffer_info_array)
                .build();
            let write_descriptor_sets = [write_descriptor_set];
            unsafe { device.update_descriptor_sets(&write_descriptor_sets, &[]) };
        }

        Ok(descriptor_sets)
    }

    pub fn reset_per_frame_descriptor_pool(&self) {
        let device = self.device.upgrade().unwrap();
        let _ = unsafe {
            device.reset_descriptor_pool(self.per_frame_pool, vk::DescriptorPoolResetFlags::empty())
        };
    }

    pub fn get_vk_descriptor_set_layouts(
        &self,
        material: &VulkanMaterial,
    ) -> [vk::DescriptorSetLayout; 3] {
        let per_material_layout = self.get_per_material_descriptor_layout(material);
        [self.per_frame_layout, self.dub_descriptor_manager.layout().vk_layout(), per_material_layout]
    }

    fn create_per_frame_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let uniform_pool_size = vk::DescriptorPoolSize::builder()
            .descriptor_count(MAX_SWAPCHAIN_IMAGE_COUNT)
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .build();

        let pool_sizes = [uniform_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT)
            .build();

        unsafe { device.create_descriptor_pool(&create_info, None) }
    }

    fn create_per_object_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let sampler_pool_size = vk::DescriptorPoolSize::builder()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build();

        let pool_sizes = [sampler_pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT)
            .build();

        unsafe { device.create_descriptor_pool(&create_info, None) }
    }

    fn create_descriptor_set_layout(
        device: &Device,
        descriptor_type: vk::DescriptorType,
        stage_flags: vk::ShaderStageFlags,
        descriptor_count: u32,
    ) -> VkResult<vk::DescriptorSetLayout> {
        let layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(descriptor_count)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .build();

        let bindings = [layout_binding];
        let create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();

        unsafe { device.create_descriptor_set_layout(&create_info, None) }
    }

    fn get_per_material_descriptor_layout(
        &self,
        material: &VulkanMaterial,
    ) -> vk::DescriptorSetLayout {
        let device = self.device.upgrade().unwrap();
        let mut per_material_layouts = self.per_material_layouts.lock().unwrap();
        if !per_material_layouts.contains_key(material.name()) {
            let layout = Self::create_descriptor_set_layout(
                &device,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
                material.textures().len() as u32,
            )
            .unwrap();
            per_material_layouts.insert(material.name().to_owned(), layout);
        }

        *per_material_layouts.get(material.name()).unwrap()
    }
}

impl Drop for DescriptorManager {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_descriptor_set_layout(self.per_frame_layout, None);
            for layout in self.per_material_layouts.lock().unwrap().values() {
                device.destroy_descriptor_set_layout(*layout, None);
            }
            device.destroy_descriptor_pool(self.per_frame_pool, None);
            device.destroy_descriptor_pool(self.per_object_pool, None);
        }
    }
}

pub struct DynamicUniformBufferDescriptorManager {
    device: Weak<Device>,
    pool: DescriptorPool,
    layout: DescriptorSetLayout,
}

impl DynamicUniformBufferDescriptorManager {
    pub fn new(device: &Rc<Device>) -> Self {
        let pool = DescriptorPool::new(
            device,
            &[DescriptorPoolCreateInfo {
                ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                descriptor_count: 1,
            }],
        );
        let layout = DescriptorSetLayout::new(
            device,
            vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
            vk::ShaderStageFlags::VERTEX,
            1,
        );

        Self {
            device: Rc::downgrade(device),
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
        let device = self.device.upgrade().unwrap();
        let layouts = [self.layout.vk_layout()];
        let create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool.vk_pool())
            .set_layouts(&layouts)
            .build();
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&create_info) }.unwrap();

        let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(dynamic_uniform_buffer.vk_buffer())
            .offset(0)
            .range(dynamic_uniform_buffer.element_size() as u64)
            .build();

        let buffer_info_array = [buffer_info];
        let write_descriptor_set = vk::WriteDescriptorSet::builder()
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .dst_set(descriptor_sets[0])
            .dst_binding(0)
            .dst_array_element(0)
            .buffer_info(&buffer_info_array)
            .build();
        let write_descriptor_sets = [write_descriptor_set];
        unsafe { device.update_descriptor_sets(&write_descriptor_sets, &[]) };

        descriptor_sets[0]
    }
}
