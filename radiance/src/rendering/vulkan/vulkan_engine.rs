use super::buffer::{Buffer, BufferType};
use super::creation_helpers;
use super::descriptor_manager::DescriptorManager;
use super::helpers;
use super::image::Image;
use super::render_object::VulkanRenderObject;
use super::swapchain::SwapChain;
use super::uniform_buffer_mvp::UniformBufferMvp;
use crate::math::Transform;
use crate::math::{Mat44, Vec3};
use crate::rendering::RenderObject;
use crate::rendering::{RenderingEngine, Window};
use crate::scene::{Camera, Entity, Scene};
use ash::extensions::ext::DebugReport;
use ash::prelude::VkResult;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::{vk, Device, Entry, Instance};
use std::error::Error;
use std::f32::consts::PI;
use std::iter::Iterator;
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
    command_pool: Rc<vk::CommandPool>,
    debug_callback: vk::DebugReportCallbackEXT,

    descriptor_manager: Option<DescriptorManager>,

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

        let mut descriptor_manager = DescriptorManager::new(&device)?;
        let swapchain = SwapChain::new(
            &instance,
            &device,
            &command_pool,
            physical_device,
            surface,
            capabilities,
            format,
            present_mode,
            &mut descriptor_manager,
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
            descriptor_manager: Some(descriptor_manager),
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
    pub fn device(&self) -> &Rc<Device> {
        &self.device
    }

    pub fn descriptor_manager_mut(&mut self) -> &mut DescriptorManager {
        (&mut self.descriptor_manager).as_mut().unwrap()
    }

    fn record_command_buffers(&mut self, scene: &mut Scene) {
        let swapchain = self.swapchain.as_mut().unwrap();
        let objects: Vec<&mut VulkanRenderObject> = scene
            .entities_mut()
            .iter_mut()
            .filter_map(|e| e.get_component_mut::<VulkanRenderObject>())
            .collect();

        swapchain.record_command_buffers(&objects).unwrap();
    }

    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let _ = self.device.device_wait_idle();
        }

        self.swapchain = None;
        let capabilities = self.get_capabilities()?;
        let descriptor_manager = (&mut self.descriptor_manager).as_mut().unwrap();
        self.swapchain = Some(SwapChain::new(
            &self.instance,
            &self.device,
            &self.command_pool,
            self.physical_device,
            self.surface,
            capabilities,
            self.format,
            self.present_mode,
            descriptor_manager,
        )?);

        Ok(())
    }

    pub fn create_device_buffer_with_data<T>(
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
            self,
        )
    }

    pub fn create_staging_buffer_with_data<T>(&self, data: &[T]) -> Result<Buffer, Box<dyn Error>> {
        Buffer::new_staging_buffer_with_data::<T>(
            &self.instance,
            &self.device,
            self.physical_device,
            data,
        )
    }

    pub fn create_image(&self, width: u32, height: u32) -> Result<Image, Box<dyn Error>> {
        Image::new(
            &self.instance,
            &self.device,
            self.physical_device,
            width,
            height,
        )
    }

    pub fn run_commands_one_shot<F: FnOnce(&Rc<Device>, &vk::CommandBuffer)>(
        &self,
        record_command: F,
    ) -> VkResult<()> {
        let command_buffers = {
            let allocation_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(*self.command_pool.as_ref())
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1)
                .build();
            unsafe { self.device.allocate_command_buffers(&allocation_info)? }
        };

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe {
            self.device
                .begin_command_buffer(command_buffers[0], &begin_info)?
        };

        record_command(&self.device, &command_buffers[0]);

        unsafe {
            self.device.end_command_buffer(command_buffers[0])?;

            let submit_info = vk::SubmitInfo::builder()
                .command_buffers(&command_buffers)
                .build();
            self.device
                .queue_submit(self.queue, &[submit_info], vk::Fence::default())?;
            self.device.queue_wait_idle(self.queue)?;
            self.device
                .free_command_buffers(*self.command_pool.as_ref(), &command_buffers);
        }

        Ok(())
    }

    fn render_objects(&mut self, entities: &Vec<Entity>) -> Result<(), Box<dyn Error>> {
        let swapchain = self.swapchain.as_mut().unwrap();
        unsafe {
            let (image_index, _) = swapchain
                .acquire_next_image(
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

                swapchain.update_ubo(image_index as usize, &[ubo])?;
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
                let ret = swapchain.present(image_index, self.queue, &wait_semaphores);

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
            self.swapchain = None;
            self.descriptor_manager = None;
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
