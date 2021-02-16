use super::descriptor_managers::DescriptorManager;
use super::helpers;
use super::render_object::VulkanRenderObject;
use super::swapchain::SwapChain;
use super::{adhoc_command_runner::AdhocCommandRunner, device::Device};
use super::{creation_helpers, instance::Instance};
use super::{
    factory::VulkanComponentFactory,
    uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer},
};
use crate::math::Mat44;
use crate::scene::{entity_get_component, Scene};
use crate::{
    imgui::{ImguiContext, ImguiFrame},
    rendering::{ComponentFactory, RenderingComponent, RenderingEngine, Window},
};
use ash::extensions::ext::DebugReport;
use ash::{vk, Entry};
use std::iter::Iterator;
use std::rc::Rc;
use std::sync::Arc;
use std::{cell::RefCell, error::Error};

pub struct VulkanRenderingEngine {
    entry: Rc<Entry>,
    instance: Rc<Instance>,
    physical_device: vk::PhysicalDevice,
    device: Rc<Device>,
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
    component_factory: Rc<VulkanComponentFactory>,

    surface_entry: ash::extensions::khr::Surface,
    debug_entry: ash::extensions::ext::DebugReport,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,

    imgui_context: Rc<RefCell<ImguiContext>>,
}

impl RenderingEngine for VulkanRenderingEngine {
    fn render(&mut self, scene: &mut dyn Scene, ui_frame: ImguiFrame) {
        if self.swapchain.is_none() {
            self.recreate_swapchain().unwrap();
        }

        self.dub_manager().update_do(|updater| {
            for entity in scene.entities() {
                if let Some(rc) = entity_get_component::<RenderingComponent>(entity) {
                    for ro in rc.render_objects() {
                        if let Some(vro) = ro.downcast_ref::<VulkanRenderObject>() {
                            updater(vro.dub_index(), entity.world_transform().matrix());
                        }
                    }
                }
            }
        });

        match self.render_objects(scene, ui_frame) {
            Ok(()) => (),
            Err(err) => println!("{}", err),
        }
    }

    fn view_extent(&self) -> (u32, u32) {
        (
            self.get_capabilities().unwrap().current_extent.width,
            self.get_capabilities().unwrap().current_extent.height,
        )
    }

    fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        self.component_factory.as_component_factory()
    }
}

impl VulkanRenderingEngine {
    pub fn new(
        window: &Window,
        imgui_context: Rc<RefCell<ImguiContext>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Rc::new(Entry::new().unwrap());
        let instance = Rc::new(Instance::new(entry.clone()));
        let physical_device = creation_helpers::get_physical_device(instance.vk_instance())?;

        let surface_entry =
            ash::extensions::khr::Surface::new(entry.as_ref(), instance.vk_instance());
        let surface =
            creation_helpers::create_surface(entry.as_ref(), instance.vk_instance(), &window)?;

        let graphics_queue_family_index = creation_helpers::get_graphics_queue_family_index(
            instance.vk_instance(),
            physical_device,
            &surface_entry,
            surface,
        )?;

        let device = Rc::new(Device::new(
            instance.clone(),
            physical_device,
            graphics_queue_family_index,
        ));

        let allocator = Rc::new({
            let create_info = vk_mem::AllocatorCreateInfo {
                physical_device,
                device: device.vk_device().clone(),
                instance: instance.vk_instance().clone(),
                flags: vk_mem::AllocatorCreateFlags::NONE,
                preferred_large_heap_block_size: 0,
                frame_in_use_count: 0,
                heap_size_limits: None,
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

        let queue = device.get_device_queue(graphics_queue_family_index, 0);
        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(
                    vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                        | vk::CommandPoolCreateFlags::TRANSIENT,
                )
                .queue_family_index(graphics_queue_family_index)
                .build();
            device.create_command_pool(&create_info)?
        };

        let descriptor_manager = Rc::new(DescriptorManager::new(device.clone()).unwrap());
        let min_uniform_buffer_alignment = instance
            .get_physical_device_properties(physical_device)
            .limits
            .min_uniform_buffer_offset_alignment;
        let dub_manager = Arc::new(DynamicUniformBufferManager::new(
            &allocator,
            descriptor_manager.dub_descriptor_manager(),
            min_uniform_buffer_alignment,
        ));

        let adhoc_command_runner =
            Rc::new(AdhocCommandRunner::new(device.clone(), command_pool, queue));
        let swapchain = SwapChain::new(
            &instance,
            device.clone(),
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
            &mut imgui_context.borrow_mut(),
        )
        .unwrap();

        let semaphore_create_info = vk::SemaphoreCreateInfo::builder().build();
        let image_available_semaphore = device.create_semaphore(&semaphore_create_info)?;
        let render_finished_semaphore = device.create_semaphore(&semaphore_create_info)?;

        let component_factory = Rc::new(VulkanComponentFactory::new(
            device.clone(),
            &allocator,
            &descriptor_manager,
            &dub_manager,
            &adhoc_command_runner,
        ));

        // DEBUG INFO
        let debug_entry = DebugReport::new(entry.as_ref(), instance.vk_instance());
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
            component_factory,
            surface_entry,
            debug_entry,
            image_available_semaphore,
            render_finished_semaphore,
            imgui_context,
        };

        return Ok(vulkan);
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
        self.device.wait_idle();

        self.swapchain = None;
        let capabilities = self.get_capabilities()?;
        self.swapchain = Some(SwapChain::new(
            &self.instance,
            self.device.clone(),
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
            &mut self.imgui_context.borrow_mut(),
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
            .filter(|e| e.visible())
            .filter_map(|e| {
                entity_get_component::<RenderingComponent>(*e)
                    .and_then(|c| Some(c.render_objects()))
            })
            .flatten()
            .filter_map(|o| o.downcast_ref())
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

            self.device
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
        self.device.wait_idle();

        Ok(())
    }

    fn drop_swapchain(&mut self) {
        self.device.wait_idle();
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
        self.device.wait_idle();
        self.swapchain = None;
        self.descriptor_manager = None;
        self.dub_manager = None;
        self.allocator = None;
        unsafe {
            self.debug_entry
                .destroy_debug_report_callback(self.debug_callback, None);
        }
        self.device.destroy_command_pool(self.command_pool);

        self.device
            .destroy_semaphore(self.image_available_semaphore);
        self.device
            .destroy_semaphore(self.render_finished_semaphore);
        unsafe {
            self.surface_entry.destroy_surface(self.surface, None);
        }
    }
}
