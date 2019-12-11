use crate::rendering::Window;
use super::Backend;
use ash::{vk, Entry, Instance, Device};
use ash::vk::{CommandPool};
use ash::version::{InstanceV1_0, DeviceV1_0};

mod platform;
mod error;
mod creation_helpers;

pub struct VulkanBackend {
    instance: Instance,
    device: Device,
    surface: vk::SurfaceKHR,
    queue: vk::Queue,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    command_pool: CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,

    surface_entry: ash::extensions::khr::Surface,
    swapchain_entry: ash::extensions::khr::Swapchain,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
}

impl Backend for VulkanBackend {
    fn test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let (image_index, _) = self.swapchain_entry.acquire_next_image(self.swapchain, u64::max_value(), self.image_available_semaphore, vk::Fence::default())?;
        
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[self.image_available_semaphore])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[self.command_buffers[image_index as usize]])
                .signal_semaphores(&[self.render_finished_semaphore])
                .build();

            self.device.queue_submit(self.queue, &[submit_info], vk::Fence::default());

            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&[self.render_finished_semaphore])
                .swapchains(&[self.swapchain])
                .image_indices(&[image_index])
                .build();

            self.swapchain_entry.queue_present(self.queue, &present_info);
        }
        Ok(())
    }
}

impl VulkanBackend {
    pub fn new(window: &Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::new()?;
        let instance = creation_helpers::create_instance(&entry)?;
        let physical_device = creation_helpers::get_physical_device(&instance)?;

        let surface_entry = ash::extensions::khr::Surface::new(&entry, &instance);
        let surface = creation_helpers::create_surface(&entry, &instance, &window)?;

        let graphics_queue_family_index = creation_helpers::get_graphics_queue_family_index(&instance, physical_device, &surface_entry, surface)?;

        let device = creation_helpers::create_device(&instance, physical_device, graphics_queue_family_index)?;
        let format = creation_helpers::get_surface_format(physical_device, &surface_entry, surface)?;
        let present_mode = creation_helpers::get_present_mode(physical_device, &surface_entry, surface)?;
        let capabilities = unsafe {
            surface_entry.get_physical_device_surface_capabilities(physical_device, surface)?
        };

        let swapchain_entry = ash::extensions::khr::Swapchain::new(&instance, &device);
        let swapchain = creation_helpers::create_swapchain(&swapchain_entry, surface, capabilities, format, present_mode)?;
        let swapchain_images = unsafe { swapchain_entry.get_swapchain_images(swapchain)? };
        let swapchain_image_views = creation_helpers::create_image_views(&device, &swapchain_images, format)?;

        let queue = unsafe { device.get_device_queue(graphics_queue_family_index, 0) };
        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER | vk::CommandPoolCreateFlags::TRANSIENT)
                .queue_family_index(graphics_queue_family_index)
                .build();
            unsafe { device.create_command_pool(&create_info, None)? }
        };

        let render_pass = creation_helpers::create_render_pass(&device, format)?;
        let pipeline_layout = creation_helpers::create_pipeline_layout(&device)?;
        let pipeline = creation_helpers::create_pipeline(&device, render_pass, pipeline_layout, &capabilities.current_extent).map_err(|(_, e)| e)?[0];

        let framebuffers = creation_helpers::create_framebuffers(&device, &swapchain_image_views, &capabilities.current_extent, render_pass)?;

        let command_buffers = {
            let create_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .command_buffer_count(framebuffers.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();
            unsafe { device.allocate_command_buffers(&create_info)? }
        };

        for (command_buffer, framebuffer) in (&command_buffers).into_iter().zip(&framebuffers) {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                .build();
            unsafe {
                device.begin_command_buffer(*command_buffer, &begin_info)?;
            }

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(render_pass)
                .framebuffer(*framebuffer)
                .render_area(
                    vk::Rect2D::builder()
                        .offset(vk::Offset2D::builder().x(0).y(0).build())
                        .extent(capabilities.current_extent)
                        .build())
                .clear_values(&[vk::ClearValue { color: vk::ClearColorValue { float32: [0f32, 0f32, 0f32, 1f32] } }])
                .build();

            unsafe {
                device.cmd_begin_render_pass(*command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE );
                device.cmd_bind_pipeline(*command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
                device.cmd_draw(*command_buffer, 3, 1, 0, 0);
                device.cmd_end_render_pass(*command_buffer);
                device.end_command_buffer(*command_buffer)?;
            }
        }

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder().build();
        let image_available_semaphore = unsafe {
            device.create_semaphore(&semaphore_create_info, None)?
        };
        let render_finished_semaphore = unsafe {
            device.create_semaphore(&semaphore_create_info, None)?
        };

        Ok(Self {
            instance,
            device,
            surface,
            queue,
            swapchain,
            swapchain_images,
            swapchain_image_views,
            command_pool,
            command_buffers,
            render_pass,
            pipeline_layout,
            pipeline,
            framebuffers,
            surface_entry,
            swapchain_entry,
            image_available_semaphore,
            render_finished_semaphore
        })
    }
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle();
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_pipeline(self.pipeline, None);
            
            self.device.free_command_buffers(self.command_pool, &self.command_buffers);
            self.device.destroy_command_pool(self.command_pool, None);

            for buffer in &self.framebuffers {
                self.device.destroy_framebuffer(*buffer, None);
            }

            for view in &self.swapchain_image_views {
                self.device.destroy_image_view(*view, None);
            }

            self.device.destroy_semaphore(self.image_available_semaphore, None);
            self.device.destroy_semaphore(self.render_finished_semaphore, None);

            self.swapchain_entry.destroy_swapchain(self.swapchain, None);
            self.surface_entry.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
