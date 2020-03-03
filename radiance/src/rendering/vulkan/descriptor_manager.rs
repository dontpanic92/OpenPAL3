use super::buffer::Buffer;
use super::descriptor_sets::DescriptorSets;
use crate::rendering::vulkan::material::VulkanMaterial;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::Mutex;

const MAX_DESCRIPTOR_SET_COUNT: u32 = 40960;
const MAX_DESCRIPTOR_COUNT: u32 = 4096;
const MAX_SWAPCHAIN_IMAGE_COUNT: u32 = 4;

pub struct DescriptorManager {
    device: Weak<Device>,
    per_frame_pool: vk::DescriptorPool,
    per_object_pool: vk::DescriptorPool,
    per_frame_layout: vk::DescriptorSetLayout,
    per_material_layouts: Arc<Mutex<HashMap<String, vk::DescriptorSetLayout>>>,
}

impl DescriptorManager {
    pub fn new(device: &Rc<Device>) -> VkResult<Self> {
        let per_frame_pool = Self::create_per_frame_descriptor_pool(device)?;
        let per_object_pool = Self::create_per_object_descriptor_pool(device)?;
        let per_frame_layout = Self::create_descriptor_set_layout(
            device,
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::ShaderStageFlags::VERTEX,
            1,
        )?;

        Ok(Self {
            device: Rc::downgrade(device),
            per_frame_pool,
            per_object_pool,
            per_frame_layout,
            per_material_layouts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn allocate_per_object_descriptor_set(
        &self,
        material: &VulkanMaterial,
    ) -> VkResult<DescriptorSets> {
        let device = self.device.upgrade().unwrap();
        let layout = self.get_per_material_descriptor_layout(material);

        DescriptorSets::new_per_object(&device, self.per_object_pool, layout, material.textures())
    }

    pub fn allocate_per_frame_descriptor_sets(
        &self,
        uniform_buffers: &[Buffer],
    ) -> VkResult<DescriptorSets> {
        let device = self.device.upgrade().unwrap();
        DescriptorSets::new_per_frame(
            &device,
            self.per_frame_pool,
            self.per_frame_layout,
            uniform_buffers,
        )
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
    ) -> [vk::DescriptorSetLayout; 2] {
        let per_material_layout = self.get_per_material_descriptor_layout(material);
        [self.per_frame_layout, per_material_layout]
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
