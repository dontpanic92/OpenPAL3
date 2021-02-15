use super::adhoc_command_runner::AdhocCommandRunner;
use super::buffer::Buffer;
use super::error::VulkanBackendError;
use ash::prelude::VkResult;
use ash::version::InstanceV1_0;
use ash::{vk, Instance};
use std::error::Error;
use std::rc::Rc;

pub struct Image {
    allocator: Rc<vk_mem::Allocator>,
    image: vk::Image,
    allocation: vk_mem::Allocation,
    allocation_info: vk_mem::AllocationInfo,
    format: vk::Format,
    width: u32,
    height: u32,
}

impl Image {
    pub fn new_color_image(
        allocator: &Rc<vk_mem::Allocator>,
        tex_width: u32,
        tex_height: u32,
    ) -> Result<Self, Box<dyn Error>> {
        Self::new(
            allocator,
            tex_width,
            tex_height,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        )
    }

    pub fn new_depth_image(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        allocator: &Rc<vk_mem::Allocator>,
        tex_width: u32,
        tex_height: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let format = Image::find_supported_format(
            instance,
            physical_device,
            &vec![
                vk::Format::D24_UNORM_S8_UINT,
                vk::Format::D32_SFLOAT,
                vk::Format::D32_SFLOAT_S8_UINT,
            ],
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        )?;

        Self::new(
            allocator,
            tex_width,
            tex_height,
            format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn vk_image(&self) -> vk::Image {
        self.image
    }

    pub fn vk_format(&self) -> vk::Format {
        self.format
    }

    pub fn transit_layout(
        &mut self,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        command_runner: &AdhocCommandRunner,
    ) -> VkResult<()> {
        command_runner.run_commands_one_shot(|device, command_buffer| {
            let aspect_mask = {
                match new_layout {
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => {
                        let mut aspect_mask = vk::ImageAspectFlags::DEPTH;
                        if self.format == vk::Format::D32_SFLOAT_S8_UINT
                            || self.format == vk::Format::D24_UNORM_S8_UINT
                        {
                            aspect_mask |= vk::ImageAspectFlags::STENCIL;
                        }

                        aspect_mask
                    }
                    _ => vk::ImageAspectFlags::COLOR,
                }
            };

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
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => (
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
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
                        .aspect_mask(aspect_mask)
                        .level_count(1)
                        .base_mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                )
                .src_access_mask(src_access_mask)
                .dst_access_mask(dst_access_mask)
                .build();
            device.cmd_pipeline_barrier(
                *command_buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::default(),
                &[],
                &[],
                &[barrier],
            )
        })
    }

    pub fn copy_from(
        &mut self,
        buffer: &Buffer,
        command_runner: &AdhocCommandRunner,
    ) -> VkResult<()> {
        command_runner.run_commands_one_shot(|device, command_buffer| {
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
            device.cmd_copy_buffer_to_image(
                *command_buffer,
                buffer.vk_buffer(),
                self.vk_image(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            )
        })
    }

    fn new(
        allocator: &Rc<vk_mem::Allocator>,
        tex_width: u32,
        tex_height: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
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
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .build();

        let allcation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };
        let (image, allocation, allocation_info) = allocator
            .create_image(&create_info, &allcation_create_info)
            .unwrap();

        Ok(Self {
            allocator: allocator.clone(),
            image,
            allocation,
            allocation_info,
            format,
            width: tex_width,
            height: tex_height,
        })
    }

    fn find_supported_format(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        candidates: &Vec<vk::Format>,
        tiling: vk::ImageTiling,
        features: vk::FormatFeatureFlags,
    ) -> Result<vk::Format, Box<dyn Error>> {
        for format in candidates {
            let properties =
                unsafe { instance.get_physical_device_format_properties(physical_device, *format) };
            if tiling == vk::ImageTiling::LINEAR
                && (properties.linear_tiling_features & features) == features
            {
                return Ok(*format);
            } else if tiling == vk::ImageTiling::OPTIMAL
                && (properties.optimal_tiling_features & features) == features
            {
                return Ok(*format);
            }
        }

        return Err(VulkanBackendError::NoSuitableFormatFound)?;
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        self.allocator
            .destroy_image(self.image, &self.allocation)
            .unwrap();
    }
}
