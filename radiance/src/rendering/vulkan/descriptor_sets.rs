use super::buffer::Buffer;
use super::uniform_buffer_mvp::UniformBufferMvp;
use crate::rendering::vulkan::texture::VulkanTexture;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::{Rc, Weak};

pub struct DescriptorSets {
    device: Weak<Device>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
}

impl DescriptorSets {
    pub fn new_per_frame(
        device: &Rc<Device>,
        descriptor_pool: vk::DescriptorPool,
        layout: vk::DescriptorSetLayout,
        uniform_buffers: &[Buffer],
    ) -> VkResult<Self> {
        let layouts = vec![layout; uniform_buffers.len()];
        let create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(layouts.as_slice())
            .build();
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&create_info) }?;

        for i in 0..uniform_buffers.len() {
            let uniform_buffer_info = vk::DescriptorBufferInfo::builder()
                .buffer(uniform_buffers[i].vk_buffer())
                .offset(0)
                .range(std::mem::size_of::<UniformBufferMvp>() as u64)
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

        Ok(Self {
            device: Rc::downgrade(device),
            descriptor_pool,
            descriptor_sets,
        })
    }

    pub fn new_per_object(
        device: &Rc<Device>,
        descriptor_pool: vk::DescriptorPool,
        layout: vk::DescriptorSetLayout,
        textures: &[VulkanTexture],
    ) -> VkResult<Self> {
        let layouts = [layout];
        let create_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts)
            .build();
        let descriptor_sets = unsafe { device.allocate_descriptor_sets(&create_info) }?;

        let image_info: Vec<vk::DescriptorImageInfo> = textures
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

        Ok(Self {
            device: Rc::downgrade(device),
            descriptor_pool,
            descriptor_sets,
        })
    }

    pub fn vk_descriptor_set(&self) -> &Vec<vk::DescriptorSet> {
        &self.descriptor_sets
    }
}
