use super::buffer::Buffer;
use super::creation_helpers;
use crate::rendering::backend::RenderingBackend;
use crate::rendering::Window;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk::CommandPool;
use ash::{vk, Device, Entry, Instance};
use std::error::Error;
use std::ops::Deref;
use std::rc::{Rc, Weak};

use crate::rendering::core_engine::{indices, vertices};

pub struct VulkanRenderingBackend {
    entry: Entry,
    instance: Instance,
    physical_device: vk::PhysicalDevice,
    device: Rc<Device>,
    surface: vk::SurfaceKHR,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    queue: vk::Queue,
    swapchain: Option<SwapChain>,
    command_pool: CommandPool,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,

    surface_entry: ash::extensions::khr::Surface,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
}

impl RenderingBackend for VulkanRenderingBackend {
    fn new(window: &Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::new()?;
        let instance = creation_helpers::create_instance(&entry)?;
        let physical_device = creation_helpers::get_physical_device(&instance)?;

        let surface_entry = ash::extensions::khr::Surface::new(&entry, &instance);
        let surface = creation_helpers::create_surface(&entry, &instance, &window)?;

        let graphics_queue_family_index = creation_helpers::get_graphics_queue_family_index(
            &instance,
            physical_device,
            &surface_entry,
            surface,
        )?;

        let device = Rc::new(creation_helpers::create_device(
            &instance,
            physical_device,
            graphics_queue_family_index,
        )?);
        let format =
            creation_helpers::get_surface_format(physical_device, &surface_entry, surface)?;
        let present_mode =
            creation_helpers::get_present_mode(physical_device, &surface_entry, surface)?;
        let capabilities = unsafe {
            surface_entry.get_physical_device_surface_capabilities(physical_device, surface)?
        };

        let queue = unsafe { device.get_device_queue(graphics_queue_family_index, 0) };
        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(
                    vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                        | vk::CommandPoolCreateFlags::TRANSIENT,
                )
                .queue_family_index(graphics_queue_family_index)
                .build();
            unsafe { device.create_command_pool(&create_info, None)? }
        };

        let vertex_buffer = Buffer::new_vertex_buffer(
            &instance,
            &device,
            physical_device,
            &vertices,
            command_pool,
            queue,
        )?;
        let index_buffer = Buffer::new_index_buffer(
            &instance,
            &device,
            physical_device,
            &indices,
            command_pool,
            queue,
        )?;

        let swapchain = SwapChain::new(
            &instance,
            Rc::downgrade(&device),
            surface,
            capabilities,
            format,
            present_mode,
            command_pool,
        )?;

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder().build();
        let image_available_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };
        let render_finished_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };

        let mut vulkan = Self {
            entry,
            instance,
            physical_device,
            device,
            surface,
            format,
            present_mode,
            queue,
            command_pool,
            vertex_buffer: Some(vertex_buffer),
            index_buffer: Some(index_buffer),
            swapchain: Some(swapchain),
            surface_entry,
            image_available_semaphore,
            render_finished_semaphore,
        };

        vulkan.record_command_buffers()?;
        return Ok(vulkan);
    }

    fn test(&mut self) -> Result<(), Box<dyn Error>> {
        let swapchain = self.swapchain.as_ref().unwrap();
        unsafe {
            let (image_index, _) = swapchain.entry.acquire_next_image(
                swapchain.handle,
                u64::max_value(),
                self.image_available_semaphore,
                vk::Fence::default(),
            )?;
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[self.image_available_semaphore])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[swapchain.command_buffers[image_index as usize]])
                .signal_semaphores(&[self.render_finished_semaphore])
                .build();

            self.device
                .queue_submit(self.queue, &[submit_info], vk::Fence::default())?;

            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&[self.render_finished_semaphore])
                .swapchains(&[swapchain.handle])
                .image_indices(&[image_index])
                .build();

            match swapchain.entry.queue_present(self.queue, &present_info) {
                Ok(true) => (),
                Ok(false) => self.recreate_swapchain()?,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_swapchain()?,
                Err(x) => return Err(Box::new(x) as Box<dyn Error>),
            };

            // Not an optimized way
            let _ = self.device.device_wait_idle();
        }
        Ok(())
    }
}

impl VulkanRenderingBackend {
    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let _ = self.device.device_wait_idle();
        }

        drop(self.swapchain.take());
        self.swapchain = Some(SwapChain::new(
            &self.instance,
            Rc::downgrade(&self.device),
            self.surface,
            self.get_capabilities()?,
            self.format,
            self.present_mode,
            self.command_pool,
        )?);

        self.record_command_buffers()?;

        Ok(())
    }

    fn record_command_buffers(&mut self) -> Result<(), vk::Result> {
        let swapchain = self.swapchain.as_ref().unwrap();
        for (command_buffer, framebuffer) in (&swapchain.command_buffers)
            .into_iter()
            .zip(&swapchain.framebuffers)
        {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                .build();
            unsafe {
                self.device
                    .begin_command_buffer(*command_buffer, &begin_info)?;
            }

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(swapchain.render_pass)
                .framebuffer(*framebuffer)
                .render_area(
                    vk::Rect2D::builder()
                        .offset(vk::Offset2D::builder().x(0).y(0).build())
                        .extent(self.get_capabilities()?.current_extent)
                        .build(),
                )
                .clear_values(&[vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0f32, 0f32, 0f32, 1f32],
                    },
                }])
                .build();

            unsafe {
                self.device.cmd_begin_render_pass(
                    *command_buffer,
                    &render_pass_begin_info,
                    vk::SubpassContents::INLINE,
                );
                self.device.cmd_bind_pipeline(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    swapchain.pipeline,
                );
                self.device.cmd_bind_vertex_buffers(
                    *command_buffer,
                    0,
                    &[self.vertex_buffer.as_ref().unwrap().buffer()],
                    &[0],
                );
                self.device.cmd_bind_index_buffer(
                    *command_buffer,
                    self.index_buffer.as_ref().unwrap().buffer(),
                    0,
                    vk::IndexType::UINT32,
                );
                self.device
                    .cmd_draw_indexed(*command_buffer, indices.len() as u32, 1, 0, 0, 0);
                self.device.cmd_end_render_pass(*command_buffer);
                self.device.end_command_buffer(*command_buffer)?;
            }
        }

        Ok(())
    }

    fn get_capabilities(&self) -> ash::prelude::VkResult<vk::SurfaceCapabilitiesKHR> {
        unsafe {
            self.surface_entry
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
        }
    }
}

impl Drop for VulkanRenderingBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            drop(self.vertex_buffer.take());
            drop(self.index_buffer.take());
            drop(self.swapchain.take());
            self.device.destroy_command_pool(self.command_pool, None);

            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device
                .destroy_semaphore(self.render_finished_semaphore, None);

            self.surface_entry.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

struct SwapChain {
    device: Weak<Device>,
    command_pool: vk::CommandPool,

    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    command_buffers: Vec<vk::CommandBuffer>,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,

    entry: ash::extensions::khr::Swapchain,
}

impl SwapChain {
    fn new(
        instance: &Instance,
        device: Weak<Device>,
        surface: vk::SurfaceKHR,
        capabilities: vk::SurfaceCapabilitiesKHR,
        format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
        command_pool: vk::CommandPool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let rc_device = device.upgrade().unwrap();

        let entry = ash::extensions::khr::Swapchain::new(instance, rc_device.deref());

        let handle = creation_helpers::create_swapchain(
            &entry,
            surface,
            capabilities,
            format,
            present_mode,
        )?;
        let images = unsafe { entry.get_swapchain_images(handle)? };
        let image_views = creation_helpers::create_image_views(&rc_device, &images, format)?;

        let render_pass = creation_helpers::create_render_pass(&rc_device, format)?;
        let pipeline_layout = creation_helpers::create_pipeline_layout(&rc_device)?;
        let pipeline = creation_helpers::create_pipeline(
            &rc_device,
            render_pass,
            pipeline_layout,
            &capabilities.current_extent,
        )?[0];

        let framebuffers = creation_helpers::create_framebuffers(
            &rc_device,
            &image_views,
            &capabilities.current_extent,
            render_pass,
        )?;

        let command_buffers = {
            let create_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .command_buffer_count(framebuffers.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();
            unsafe { rc_device.allocate_command_buffers(&create_info)? }
        };

        Ok(Self {
            device,
            command_pool,
            handle,
            images,
            image_views,
            command_buffers,
            render_pass,
            pipeline_layout,
            pipeline,
            framebuffers,
            entry,
        })
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            for buffer in &self.framebuffers {
                rc_device.destroy_framebuffer(*buffer, None);
            }

            rc_device.free_command_buffers(self.command_pool, &self.command_buffers);
            rc_device.destroy_pipeline_layout(self.pipeline_layout, None);
            rc_device.destroy_render_pass(self.render_pass, None);
            rc_device.destroy_pipeline(self.pipeline, None);

            for view in &self.image_views {
                rc_device.destroy_image_view(*view, None);
            }

            self.entry.destroy_swapchain(self.handle, None);
        }
    }
}
