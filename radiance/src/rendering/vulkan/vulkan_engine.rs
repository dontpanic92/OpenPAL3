use super::adhoc_command_runner::AdhocCommandRunner;
use super::creation_helpers;
use super::descriptor_managers::DescriptorManager;
use super::helpers;
use super::render_object::VulkanRenderObject;
use super::swapchain::SwapChain;
use super::uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer};
use crate::math::Mat44;
use crate::rendering::RenderObject;
use crate::rendering::{
    imgui::{ImguiContext, ImguiFrame},
    RenderingEngine, Window,
};
use crate::scene::{entity_get_component, Scene};
use ash::extensions::ext::DebugReport;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::{vk, Device, Entry, Instance};
use std::any::TypeId;
use std::error::Error;
use std::iter::Iterator;
use std::rc::Rc;
use std::sync::Arc;

pub struct VulkanRenderingEngine {
    entry: Entry,
    instance: Rc<Instance>,
    physical_device: vk::PhysicalDevice,
    device: Option<Rc<Device>>,
    allocator: Option<Rc<vk_mem::Allocator>>,
    surface: vk::SurfaceKHR,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    queue: vk::Queue,
    swapchain: Option<SwapChain>,
    command_pool: vk::CommandPool,
    debug_callback: vk::DebugReportCallbackEXT,

    descriptor_manager: Option<Rc<DescriptorManager>>,
    dub_manager: Option<Arc<DynamicUniformBufferManager>>,
    adhoc_command_runner: Rc<AdhocCommandRunner>,

    surface_entry: ash::extensions::khr::Surface,
    debug_entry: ash::extensions::ext::DebugReport,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,

    imgui: ImguiContext,
}

impl RenderingEngine for VulkanRenderingEngine {
    fn new(window: &Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::new().unwrap();
        let instance = Rc::new(creation_helpers::create_instance(&entry)?);
        let physical_device = creation_helpers::get_physical_device(&instance)?;

        let surface_entry = ash::extensions::khr::Surface::new(&entry, instance.as_ref());
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

        let allocator = Rc::new({
            let create_info = vk_mem::AllocatorCreateInfo {
                physical_device,
                device: (*device).clone(),
                instance: instance.as_ref().clone(),
                ..Default::default()
            };

            vk_mem::Allocator::new(&create_info).unwrap()
        });

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

        let descriptor_manager = Rc::new(DescriptorManager::new(&device).unwrap());
        let min_uniform_buffer_alignment = unsafe {
            instance
                .get_physical_device_properties(physical_device)
                .limits
                .min_uniform_buffer_offset_alignment
        };
        let dub_manager = Arc::new(DynamicUniformBufferManager::new(
            &device,
            &allocator,
            descriptor_manager.dub_descriptor_manager(),
            min_uniform_buffer_alignment,
        ));

        let adhoc_command_runner = Rc::new(AdhocCommandRunner::new(&device, command_pool, queue));
        let mut imgui = ImguiContext::new(
            capabilities.current_extent.width as f32,
            capabilities.current_extent.height as f32,
        );
        let swapchain = SwapChain::new(
            &instance,
            &device,
            &allocator,
            command_pool,
            physical_device,
            queue,
            surface,
            capabilities,
            format,
            present_mode,
            &descriptor_manager,
            &adhoc_command_runner,
            &mut imgui,
        )
        .unwrap();

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder().build();
        let image_available_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };
        let render_finished_semaphore =
            unsafe { device.create_semaphore(&semaphore_create_info, None)? };

        // DEBUG INFO
        let debug_entry = DebugReport::new(&entry, instance.as_ref());
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
            device: Some(device),
            allocator: Some(allocator),
            surface,
            format,
            present_mode,
            queue,
            command_pool,
            swapchain: Some(swapchain),
            debug_callback,
            descriptor_manager: Some(descriptor_manager),
            dub_manager: Some(dub_manager),
            adhoc_command_runner,
            surface_entry,
            debug_entry,
            image_available_semaphore,
            render_finished_semaphore,
            imgui,
        };

        return Ok(vulkan);
    }

    fn render(&mut self, scene: &mut dyn Scene, ui_frame: ImguiFrame) {
        if self.swapchain.is_none() {
            self.recreate_swapchain().unwrap();
        }

        for entity in scene.entities_mut() {
            unsafe {
                entity.component_do2(
                    TypeId::of::<RenderObject>(),
                    TypeId::of::<VulkanRenderObject>(),
                    &|ro_any, vro_any, ent| {
                        if ro_any.is_none() {
                            ent.remove_component(TypeId::of::<VulkanRenderObject>());
                            return;
                        }
                        let ro = ro_any.unwrap().downcast_mut::<RenderObject>().unwrap();
                        let vro = vro_any
                            .and_then(|v| {
                                let vro = v.downcast_mut::<VulkanRenderObject>().unwrap();
                                if !vro.compatible_with(&ro) {
                                    ent.remove_component(TypeId::of::<VulkanRenderObject>());
                                    None
                                } else {
                                    Some(vro)
                                }
                            })
                            .or_else(|| {
                                let object = VulkanRenderObject::new(
                                    ro,
                                    self.device(),
                                    self.allocator(),
                                    self.command_runner(),
                                    self.dub_manager(),
                                    self.descriptor_manager(),
                                )
                                .unwrap();

                                ent.add_component(Box::new(object));
                                Some(
                                    ent.get_component_mut(TypeId::of::<VulkanRenderObject>())
                                        .unwrap()
                                        .downcast_mut::<VulkanRenderObject>()
                                        .unwrap(),
                                )
                            })
                            .unwrap();

                        if ro.is_dirty {
                            vro.update(ro);
                        }
                    },
                );
            }
        }

        self.dub_manager().update_do(|updater| {
            for entity in scene.entities() {
                if let Some(vro) = entity_get_component::<VulkanRenderObject>(entity.as_ref()) {
                    updater(vro.dub_index(), entity.transform().matrix());
                }
            }
        });

        match self.render_objects(scene, ui_frame) {
            Ok(()) => (),
            Err(err) => println!("{}", err),
        }
    }

    fn gui_context_mut(&mut self) -> &mut ImguiContext {
        &mut self.imgui
    }

    fn view_extent(&self) -> (u32, u32) {
        (
            self.get_capabilities().unwrap().current_extent.width,
            self.get_capabilities().unwrap().current_extent.height,
        )
    }

    fn scene_loaded(&mut self, scene: &mut dyn Scene) {
        for e in scene.entities_mut() {
            match entity_get_component::<RenderObject>(e.as_ref()) {
                None => continue,
                Some(render_object) => {
                    let object = VulkanRenderObject::new(
                        render_object,
                        self.device(),
                        self.allocator(),
                        self.command_runner(),
                        self.dub_manager(),
                        self.descriptor_manager(),
                    )
                    .unwrap();
                    e.add_component(Box::new(object));
                }
            }
        }
    }
}

impl VulkanRenderingEngine {
    pub fn device(&self) -> &Rc<Device> {
        self.device.as_ref().unwrap()
    }

    pub fn allocator(&self) -> &Rc<vk_mem::Allocator> {
        self.allocator.as_ref().unwrap()
    }

    pub fn dub_manager(&self) -> &Arc<DynamicUniformBufferManager> {
        self.dub_manager.as_ref().unwrap()
    }

    pub fn command_runner(&self) -> &Rc<AdhocCommandRunner> {
        &self.adhoc_command_runner
    }

    pub fn descriptor_manager(&self) -> &Rc<DescriptorManager> {
        &self.descriptor_manager.as_ref().unwrap()
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    pub fn vk_physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub fn vk_queue(&self) -> vk::Queue {
        self.queue
    }

    pub fn vk_command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }

    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let _ = self.device().device_wait_idle();
        }

        self.swapchain = None;
        let capabilities = self.get_capabilities()?;
        self.swapchain = Some(SwapChain::new(
            &self.instance,
            self.device(),
            self.allocator(),
            self.command_pool,
            self.physical_device,
            self.queue,
            self.surface,
            capabilities,
            self.format,
            self.present_mode,
            self.descriptor_manager(),
            &self.adhoc_command_runner,
            &self.imgui,
        )?);

        Ok(())
    }

    fn render_objects(
        &mut self,
        scene: &mut dyn Scene,
        ui_frame: ImguiFrame,
    ) -> Result<(), Box<dyn Error>> {
        macro_rules! swapchain {
            ( ) => {
                self.swapchain.as_mut().unwrap()
            };
        }

        let dub_manager = self.dub_manager().clone();
        unsafe {
            let (image_index, _) = swapchain!()
                .acquire_next_image(
                    u64::max_value(),
                    self.image_available_semaphore,
                    vk::Fence::default(),
                )
                .unwrap();
            let x = &|ui| scene.draw_ui(ui);
            let objects: Vec<&VulkanRenderObject> = scene
                .entities()
                .iter()
                .filter_map(|e| entity_get_component::<VulkanRenderObject>(e.as_ref()))
                .collect();
            let command_buffer = swapchain!()
                .record_command_buffers(image_index as usize, &objects, &dub_manager, ui_frame)
                .unwrap();

            // Update Per-frame Uniform Buffers
            {
                let ubo = {
                    let camera = scene.camera();
                    let view = Mat44::inversed(camera.transform().matrix());
                    let proj = camera.projection_matrix();
                    PerFrameUniformBuffer::new(&view, proj)
                };

                swapchain!().update_ubo(image_index as usize, &[ubo]);
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

                self.device()
                    .queue_submit(self.queue, &[submit_info], vk::Fence::default())?;
            }

            // Present
            {
                let wait_semaphores = [self.render_finished_semaphore];
                let ret = swapchain!().present(image_index, self.queue, &wait_semaphores);

                match ret {
                    Ok(false) => (),
                    Ok(true) => self.drop_swapchain(),
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.drop_swapchain(),
                    Err(x) => return Err(Box::new(x) as Box<dyn Error>),
                };
            }

            // Not an optimized way
            let _ = self.device().device_wait_idle();
        }

        Ok(())
    }

    fn drop_swapchain(&mut self) {
        let _ = unsafe { self.device().device_wait_idle() };
        self.swapchain = None;
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
            let _ = self.device().device_wait_idle();
            self.swapchain = None;
            self.descriptor_manager = None;
            self.dub_manager = None;
            self.allocator = None;
            self.debug_entry
                .destroy_debug_report_callback(self.debug_callback, None);
            self.device().destroy_command_pool(self.command_pool, None);

            self.device()
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device()
                .destroy_semaphore(self.render_finished_semaphore, None);

            self.surface_entry.destroy_surface(self.surface, None);
            self.device().destroy_device(None);

            self.device = None;
            self.instance.destroy_instance(None);
        }
    }
}
