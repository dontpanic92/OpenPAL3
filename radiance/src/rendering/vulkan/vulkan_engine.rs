use super::buffer::{Buffer, BufferType};
use super::creation_helpers;
use super::helpers;
use super::render_object::VulkanRenderObject;
use super::uniform_buffer_mvp::UniformBufferMvp;
use crate::math::Transform;
use crate::math::{Mat44, Vec3};
use crate::rendering::RenderObject;
use crate::rendering::{RenderingEngine, Window};
use crate::scene::{Camera, Entity, Scene};
use ash::extensions::ext::DebugReport;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk::{CommandPool, PipelineBindPoint};
use ash::{vk, Device, Entry, Instance};
use core::borrow::Borrow;
use std::error::Error;
use std::f32::consts::PI;
use std::iter::Iterator;
use std::ops::Deref;
use std::rc::{Rc, Weak};

pub struct VulkanRenderingEngine {
    entry: Entry,
    instance: Instance,
    physical_device: vk::PhysicalDevice,
    device: Rc<Device>,
    surface: vk::SurfaceKHR,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    queue: vk::Queue,
    swapchain: Option<SwapChain>,
    command_pool: Rc<CommandPool>,
    debug_callback: vk::DebugReportCallbackEXT,

    surface_entry: ash::extensions::khr::Surface,
    debug_entry: ash::extensions::ext::DebugReport,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
}

impl RenderingEngine for VulkanRenderingEngine {
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
            Rc::new(unsafe { device.create_command_pool(&create_info, None)? })
        };

        let swapchain = SwapChain::new(
            &instance,
            &device,
            &command_pool,
            physical_device,
            surface,
            capabilities,
            format,
            present_mode,
        )?;

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder().build();
        let image_available_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };
        let render_finished_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };

        // DEBUG INFO
        let debug_entry = DebugReport::new(&entry, &instance);
        let debug_callback = {
            let create_info = vk::DebugReportCallbackCreateInfoEXT::builder()
                .flags(
                    vk::DebugReportFlagsEXT::ERROR
                        | vk::DebugReportFlagsEXT::WARNING
                        | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING,
                )
                .pfn_callback(Some(helpers::debug_callback));
            unsafe { debug_entry.create_debug_report_callback(&create_info, None)? }
        };

        let vulkan = Self {
            entry,
            instance,
            physical_device,
            device,
            surface,
            format,
            present_mode,
            queue,
            command_pool,
            swapchain: Some(swapchain),
            debug_callback,
            surface_entry,
            debug_entry,
            image_available_semaphore,
            render_finished_semaphore,
        };

        return Ok(vulkan);
    }

    fn render(&mut self, scene: &mut Scene) {
        if self.swapchain.is_none() {
            self.recreate_swapchain().unwrap();
            self.record_command_buffers(scene);
        }

        match self.render_objects(scene.entities()) {
            Ok(()) => (),
            Err(err) => println!("{}", err),
        }
    }

    fn scene_loaded(&mut self, scene: &mut Scene) {
        for e in scene.entities_mut() {
            match e.get_component::<RenderObject>() {
                None => continue,
                Some(render_object) => {
                    let object = VulkanRenderObject::new(self, render_object).unwrap();
                    e.add_component::<VulkanRenderObject>(object);
                }
            }
        }

        self.record_command_buffers(scene);
    }
}

static mut Z: f32 = 0.;

impl VulkanRenderingEngine {
    pub fn device(&self) -> Weak<Device> {
        Rc::downgrade(&self.device)
    }

    pub fn command_pool(&self) -> Weak<CommandPool> {
        Rc::downgrade(&self.command_pool)
    }

    fn record_command_buffers(&mut self, scene: &mut Scene) {
        let swapchain = self.swapchain.as_mut().unwrap();
        let objects: Vec<&mut VulkanRenderObject> = scene
            .entities_mut()
            .iter_mut()
            .filter_map(|e| e.get_component_mut::<VulkanRenderObject>())
            .collect();

        swapchain.record_command_buffers(&objects);
    }

    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let _ = self.device.device_wait_idle();
        }

        self.swapchain = None;
        self.swapchain = Some(SwapChain::new(
            &self.instance,
            &self.device,
            &self.command_pool,
            self.physical_device,
            self.surface,
            self.get_capabilities()?,
            self.format,
            self.present_mode,
        )?);

        Ok(())
    }

    pub fn create_buffer<T>(
        &self,
        buffer_type: BufferType,
        data: &Vec<T>,
    ) -> Result<Buffer, Box<dyn Error>> {
        Buffer::new_device_buffer_with_data::<T>(
            &self.instance,
            &self.device,
            self.physical_device,
            data,
            buffer_type,
            self.command_pool.borrow(),
            self.queue,
        )
    }

    fn render_objects(&mut self, entities: &Vec<Entity>) -> Result<(), Box<dyn Error>> {
        let swapchain = self.swapchain.as_mut().unwrap();
        unsafe {
            let (image_index, _) = swapchain
                .entry
                .acquire_next_image(
                    swapchain.handle,
                    u64::max_value(),
                    self.image_available_semaphore,
                    vk::Fence::default(),
                )
                .unwrap();
            let command_buffer = swapchain.command_buffers()[image_index as usize];

            // Update Uniform Buffers
            {
                let ubo = {
                    let model = Mat44::new_identity();
                    let mut transform = Transform::new();
                    transform.rotate_local(&Vec3::new(0., 1., 0.), Z);
                    transform.translate_local(&Vec3::new(0., 0., 2.));
                    Z += 0.001;
                    if Z > 6.828 {
                        Z = 0.;
                    }
                    let view = Mat44::inversed(transform.matrix());
                    let cam = Camera::new(90. * PI / 180., 800. / 600., 0.1, 100.);
                    let proj = cam.projection_matrix();
                    UniformBufferMvp::new(&model, &view, proj)
                };

                swapchain.uniform_buffers[image_index as usize].copy_memory_from(&[ubo])?;
            }

            // Submit commands
            {
                let commands = [command_buffer];
                let wait_semaphores = [self.image_available_semaphore];
                let stage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                let signal_semaphores = [self.render_finished_semaphore];
                let submit_info = vk::SubmitInfo::builder()
                    .wait_semaphores(&wait_semaphores)
                    .wait_dst_stage_mask(&stage_mask)
                    .command_buffers(&commands)
                    .signal_semaphores(&signal_semaphores)
                    .build();

                self.device
                    .queue_submit(self.queue, &[submit_info], vk::Fence::default())?;
            }

            // Present
            {
                let wait_semaphores = [self.render_finished_semaphore];
                let swapchains = [swapchain.handle];
                let image_indices = [image_index];
                let present_info = vk::PresentInfoKHR::builder()
                    .wait_semaphores(&wait_semaphores)
                    .swapchains(&swapchains)
                    .image_indices(&image_indices)
                    .build();

                let ret = swapchain.entry.queue_present(self.queue, &present_info);
                match ret {
                    Ok(false) => (),
                    Ok(true) => self.swapchain = None,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.swapchain = None,
                    Err(x) => return Err(Box::new(x) as Box<dyn Error>),
                };
            }

            // Not an optimized way
            let _ = self.device.device_wait_idle();
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

impl Drop for VulkanRenderingEngine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            drop(self.swapchain.take());
            self.debug_entry
                .destroy_debug_report_callback(self.debug_callback, None);
            self.device.destroy_command_pool(*self.command_pool, None);

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
    command_pool: Weak<CommandPool>,
    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    uniform_buffers: Vec<Buffer>,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    command_buffers: Vec<vk::CommandBuffer>,
    capabilities: vk::SurfaceCapabilitiesKHR,

    entry: ash::extensions::khr::Swapchain,
}

impl SwapChain {
    fn new(
        instance: &Instance,
        device: &Rc<Device>,
        command_pool: &Rc<CommandPool>,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        capabilities: vk::SurfaceCapabilitiesKHR,
        format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
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
        let descriptor_set_layout = creation_helpers::create_descriptor_set_layout(&device)?;
        let descriptor_pool =
            creation_helpers::create_descriptor_pool(&device, images.len() as u32)?;
        let layouts = vec![descriptor_set_layout; images.len()];
        let descriptor_sets = creation_helpers::allocate_descriptor_sets(
            &device,
            images.len() as u32,
            descriptor_pool,
            layouts.as_slice(),
            uniform_buffers.as_slice(),
        )?;

        let pipeline_layout =
            creation_helpers::create_pipeline_layout(&device, descriptor_set_layout)?;
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
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
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

    pub fn record_command_buffers(
        &self,
        objects: &[&mut VulkanRenderObject],
    ) -> Result<(), vk::Result> {
        let device = self.device.upgrade().unwrap();
        for ((command_buffer, framebuffer), descriptor_set) in (&self.command_buffers)
            .into_iter()
            .zip(&self.framebuffers)
            .zip(&self.descriptor_sets)
        {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                .build();
            unsafe {
                device.reset_command_buffer(
                    *command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                );
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
                        &[vertex_buffer.buffer()],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        *command_buffer,
                        index_buffer.buffer(),
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_bind_descriptor_sets(
                        *command_buffer,
                        PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[*descriptor_set],
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
        let rc_device = self.device.upgrade().unwrap();
        let rc_command_pool = self.command_pool.upgrade().unwrap();
        unsafe {
            for buffer in &self.framebuffers {
                rc_device.destroy_framebuffer(*buffer, None);
            }

            rc_device.free_command_buffers(*rc_command_pool, &self.command_buffers);
            rc_device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            rc_device.destroy_pipeline_layout(self.pipeline_layout, None);
            rc_device.destroy_descriptor_pool(self.descriptor_pool, None);
            rc_device.destroy_render_pass(self.render_pass, None);
            rc_device.destroy_pipeline(self.pipeline, None);

            for view in &self.image_views {
                rc_device.destroy_image_view(*view, None);
            }

            self.entry.destroy_swapchain(self.handle, None);
        }
    }
}
