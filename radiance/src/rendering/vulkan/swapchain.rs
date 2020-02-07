use super::buffer::Buffer;
use super::creation_helpers;
use super::descriptor_manager::DescriptorManager;
use super::descriptor_sets::DescriptorSets;
use super::image_view::ImageView;
use super::render_object::VulkanRenderObject;
use super::uniform_buffer_mvp::UniformBufferMvp;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device, Instance};
use std::borrow::Borrow;
use std::ops::Deref;
use std::rc::{Rc, Weak};

pub struct SwapChain {
    device: Weak<Device>,
    command_pool: Weak<vk::CommandPool>,
    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<ImageView>,
    uniform_buffers: Vec<Buffer>,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    per_frame_descriptor_sets: DescriptorSets,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    command_buffers: Vec<vk::CommandBuffer>,
    capabilities: vk::SurfaceCapabilitiesKHR,

    entry: ash::extensions::khr::Swapchain,
}

impl SwapChain {
    pub fn new(
        instance: &Instance,
        device: &Rc<Device>,
        command_pool: &Rc<vk::CommandPool>,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        capabilities: vk::SurfaceCapabilitiesKHR,
        format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
        descriptor_manager: &mut DescriptorManager,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = ash::extensions::khr::Swapchain::new(instance, device.deref());
        let handle = creation_helpers::create_swapchain(
            &entry,
            surface,
            capabilities,
            format,
            present_mode,
        )?;

        let images = unsafe { entry.get_swapchain_images(handle)? };
        let image_views = creation_helpers::create_image_views(&device, &images, format)?;
        let uniform_buffers: Vec<Buffer> = (0..images.len())
            .map(|_| {
                Buffer::new_uniform_buffer(
                    &instance,
                    &device,
                    physical_device,
                    std::mem::size_of::<UniformBufferMvp>(),
                    1,
                )
                .unwrap()
            })
            .collect();

        let render_pass = creation_helpers::create_render_pass(&device, format)?;
        let per_frame_descriptor_sets =
            descriptor_manager.allocate_per_frame_descriptor_sets(uniform_buffers.as_slice())?;

        let pipeline_layout =
            creation_helpers::create_pipeline_layout(&device, descriptor_manager)?;
        let pipeline = creation_helpers::create_pipeline(
            &device,
            render_pass,
            pipeline_layout,
            &capabilities.current_extent,
        )?[0];
        let framebuffers = creation_helpers::create_framebuffers(
            &device,
            &image_views,
            &capabilities.current_extent,
            render_pass,
        )?;

        let command_buffers = {
            let create_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(*command_pool.borrow())
                .command_buffer_count(framebuffers.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();
            unsafe { device.allocate_command_buffers(&create_info)? }
        };

        Ok(Self {
            device: Rc::downgrade(device),
            command_pool: Rc::downgrade(command_pool),
            handle,
            images,
            image_views,
            uniform_buffers,
            render_pass,
            pipeline_layout,
            per_frame_descriptor_sets,
            pipeline,
            framebuffers,
            command_buffers,
            capabilities,
            entry,
        })
    }

    pub fn command_buffers(&self) -> &Vec<vk::CommandBuffer> {
        &self.command_buffers
    }

    pub fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> VkResult<(u32, bool)> {
        unsafe {
            self.entry
                .acquire_next_image(self.handle, timeout, semaphore, fence)
        }
    }

    pub fn update_ubo<T>(&mut self, image_index: usize, data: &[T]) -> VkResult<()> {
        self.uniform_buffers[image_index]
            .memory_mut()
            .copy_from(data)
    }

    pub fn present(
        &mut self,
        image_index: u32,
        queue: vk::Queue,
        wait_semaphores: &[vk::Semaphore],
    ) -> VkResult<bool> {
        let swapchains = [self.handle];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices)
            .build();

        unsafe { self.entry.queue_present(queue, &present_info) }
    }

    pub fn record_command_buffers(
        &self,
        objects: &[&mut VulkanRenderObject],
    ) -> Result<(), vk::Result> {
        let device = self.device.upgrade().unwrap();
        for ((command_buffer, framebuffer), per_frame_descriptor_set) in (&self.command_buffers)
            .into_iter()
            .zip(&self.framebuffers)
            .zip(self.per_frame_descriptor_sets.vk_descriptor_set())
        {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                .build();
            unsafe {
                device.reset_command_buffer(
                    *command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )?;
                device.begin_command_buffer(*command_buffer, &begin_info)?;
            }

            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0f32, 0f32, 0f32, 1f32],
                },
            }];
            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.render_pass)
                .framebuffer(*framebuffer)
                .render_area(
                    vk::Rect2D::builder()
                        .offset(vk::Offset2D::builder().x(0).y(0).build())
                        .extent(self.capabilities.current_extent)
                        .build(),
                )
                .clear_values(&clear_values)
                .build();

            unsafe {
                device.cmd_begin_render_pass(
                    *command_buffer,
                    &render_pass_begin_info,
                    vk::SubpassContents::INLINE,
                );
                device.cmd_bind_pipeline(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline,
                );

                for obj in objects {
                    let vertex_buffer = obj.vertex_buffer();
                    let index_buffer = obj.index_buffer();
                    device.cmd_bind_vertex_buffers(
                        *command_buffer,
                        0,
                        &[vertex_buffer.vk_buffer()],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        *command_buffer,
                        index_buffer.vk_buffer(),
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_bind_descriptor_sets(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[*per_frame_descriptor_set, obj.vk_descriptor_set()],
                        &[],
                    );
                    device.cmd_draw_indexed(
                        *command_buffer,
                        index_buffer.element_count(),
                        1,
                        0,
                        0,
                        0,
                    );
                }

                device.cmd_end_render_pass(*command_buffer);
                device.end_command_buffer(*command_buffer)?;
            }
        }
        Ok(())
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        let command_pool = self.command_pool.upgrade().unwrap();
        unsafe {
            for buffer in &self.framebuffers {
                device.destroy_framebuffer(*buffer, None);
            }

            device.free_command_buffers(*command_pool, &self.command_buffers);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_render_pass(self.render_pass, None);
            device.destroy_pipeline(self.pipeline, None);

            self.entry.destroy_swapchain(self.handle, None);
        }
    }
}
