use super::buffer::Buffer;
use super::memory::Memory;
use super::VulkanRenderingEngine;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device, Instance};
use std::error::Error;
use std::rc::{Rc, Weak};

pub struct Image {
    device: Weak<Device>,
    image: vk::Image,
    memory: Memory,
    width: u32,
    height: u32,
}

impl Image {
    pub fn new(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        tex_width: u32,
        tex_height: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(
                vk::Extent3D::builder()
                    .width(tex_width)
                    .height(tex_height)
                    .depth(1)
                    .build(),
            )
            .mip_levels(1)
            .array_layers(1)
            .format(vk::Format::R8G8B8A8_UNORM)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .build();

        let image = unsafe { device.create_image(&create_info, None)? };
        let memory = Memory::new_for_image(
            instance,
            device,
            physical_device,
            image,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        unsafe { device.bind_image_memory(image, memory.vk_device_memory(), 0)? };

        Ok(Self {
            device: Rc::downgrade(device),
            image,
            memory,
            width: tex_width,
            height: tex_height,
        })
    }

    pub fn vk_image(&self) -> vk::Image {
        self.image
    }

    pub fn transit_layout(
        &mut self,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        engine: &VulkanRenderingEngine,
    ) -> VkResult<()> {
        engine.run_commands_one_shot(|device, command_buffer| {
            let (src_access_mask, src_stage) = {
                match old_layout {
                    vk::ImageLayout::UNDEFINED => (
                        vk::AccessFlags::default(),
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                    ),
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
                        vk::AccessFlags::TRANSFER_WRITE,
                        vk::PipelineStageFlags::TRANSFER,
                    ),
                    _ => panic!("unsupported transfer source layout: {:?}", old_layout),
                }
            };

            let (dst_access_mask, dst_stage) = {
                match new_layout {
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
                        vk::AccessFlags::TRANSFER_WRITE,
                        vk::PipelineStageFlags::TRANSFER,
                    ),
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
                        vk::AccessFlags::SHADER_READ,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                    ),
                    _ => panic!("unsupported transfer source layout: {:?}", new_layout),
                }
            };

            let barrier = vk::ImageMemoryBarrier::builder()
                .old_layout(old_layout)
                .new_layout(new_layout)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(self.image)
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .base_mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                )
                .src_access_mask(src_access_mask)
                .dst_access_mask(dst_access_mask)
                .build();
            unsafe {
                device.cmd_pipeline_barrier(
                    *command_buffer,
                    src_stage,
                    dst_stage,
                    vk::DependencyFlags::default(),
                    &[],
                    &[],
                    &[barrier],
                )
            }
        })
    }

    pub fn copy_from(&mut self, buffer: &Buffer, engine: &VulkanRenderingEngine) -> VkResult<()> {
        engine.run_commands_one_shot(|device, command_buffer| {
            let region = vk::BufferImageCopy::builder()
                .image_extent(
                    vk::Extent3D::builder()
                        .width(self.width)
                        .height(self.height)
                        .depth(1)
                        .build(),
                )
                .image_offset(vk::Offset3D::builder().x(0).y(0).z(0).build())
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(
                    vk::ImageSubresourceLayers::builder()
                        .layer_count(1)
                        .base_array_layer(0)
                        .mip_level(0)
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .build(),
                )
                .build();
            unsafe {
                device.cmd_copy_buffer_to_image(
                    *command_buffer,
                    buffer.vk_buffer(),
                    self.vk_image(),
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[region],
                )
            }
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_image(self.image, None);
        }
    }
}
