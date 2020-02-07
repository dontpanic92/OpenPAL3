use super::buffer::Buffer;
use super::descriptor_sets::DescriptorSets;
use super::image_view::ImageView;
use super::sampler::Sampler;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::{Rc, Weak};

const MAX_DESCRIPTOR_SET_COUNT: u32 = 1024;
const MAX_DESCRIPTOR_COUNT: u32 = 4096;
const MAX_SWAPCHAIN_IMAGE_COUNT: u32 = 4;

pub struct DescriptorManager {
    device: Weak<Device>,
    descriptor_pool: vk::DescriptorPool,
    per_frame_layout: vk::DescriptorSetLayout,
    per_object_layout: vk::DescriptorSetLayout,
}

impl DescriptorManager {
    pub fn new(device: &Rc<Device>) -> VkResult<Self> {
        let pool = DescriptorManager::create_descriptor_pool(device)?;
        let per_frame_layout = DescriptorManager::create_descriptor_set_layout(
            device,
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::ShaderStageFlags::VERTEX,
        )?;
        let per_object_layout = DescriptorManager::create_descriptor_set_layout(
            device,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            vk::ShaderStageFlags::FRAGMENT,
        )?;

        Ok(Self {
            device: Rc::downgrade(device),
            descriptor_pool: pool,
            per_frame_layout,
            per_object_layout,
        })
    }

    pub fn allocate_per_object_descriptor_set(
        &mut self,
        image_view: &ImageView,
        sampler: &Sampler,
    ) -> VkResult<DescriptorSets> {
        let device = self.device.upgrade().unwrap();
        DescriptorSets::new_per_object(
            &device,
            self.descriptor_pool,
            self.per_object_layout,
            image_view,
            sampler,
        )
    }

    pub fn allocate_per_frame_descriptor_sets(
        &mut self,
        uniform_buffers: &[Buffer],
    ) -> VkResult<DescriptorSets> {
        let device = self.device.upgrade().unwrap();
        DescriptorSets::new_per_frame(
            &device,
            self.descriptor_pool,
            self.per_frame_layout,
            uniform_buffers,
        )
    }

    pub fn vk_descriptor_set_layouts(&self) -> [vk::DescriptorSetLayout; 2] {
        [self.per_frame_layout, self.per_object_layout]
    }

    fn create_descriptor_pool(device: &Device) -> VkResult<vk::DescriptorPool> {
        let uniform_pool_size = vk::DescriptorPoolSize::builder()
            .descriptor_count(MAX_SWAPCHAIN_IMAGE_COUNT)
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .build();
        let sampler_pool_size = vk::DescriptorPoolSize::builder()
            .descriptor_count(MAX_DESCRIPTOR_COUNT)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build();

        let pool_sizes = [uniform_pool_size, sampler_pool_size];
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
    ) -> VkResult<vk::DescriptorSetLayout> {
        let layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .build();

        let bindings = [layout_binding];
        let create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();

        unsafe { device.create_descriptor_set_layout(&create_info, None) }
    }
}

impl Drop for DescriptorManager {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_descriptor_set_layout(self.per_frame_layout, None);
            device.destroy_descriptor_set_layout(self.per_object_layout, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
