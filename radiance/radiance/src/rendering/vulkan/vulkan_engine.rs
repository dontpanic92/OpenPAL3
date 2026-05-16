use super::descriptor_managers::DescriptorManager;
use super::helpers;
use super::imgui::ImguiRenderer;
use super::render_object::VulkanRenderObject;
use super::swapchain::SwapChain;
use super::{adhoc_command_runner::AdhocCommandRunner, device::Device};
use super::{creation_helpers, instance::Instance};
use super::{
    factory::VulkanComponentFactory,
    uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer},
};
use crate::comdef::{IEntity, IScene};
use crate::math::Mat44;
use crate::rendering::{BlendMode, RenderObject, RenderingComponent};
use crate::scene::{Camera, Viewport};
use crate::{
    imgui::{ImguiContext, ImguiFrame},
    rendering::{ComponentFactory, RenderingEngine, Window},
};
use ash::ext::debug_utils::Instance as DebugUtils;
use ash::{vk, Entry};
use crosscom::ComRc;
use std::cell::Ref;
use std::ptr;
use std::rc::Rc;
use std::sync::Arc;
use std::{cell::RefCell, error::Error};

/// Maximum number of frames the CPU is allowed to record ahead of the
/// GPU. Each frame owns its own semaphores + fence; `current_frame`
/// cycles through `[0, MAX_FRAMES_IN_FLIGHT)`. This is independent of
/// the swapchain image count — the two are tied together with the
/// per-image `images_in_flight` map so a slow swapchain doesn't let two
/// in-flight CPU frames write to the same image's command buffer.
pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct VulkanRenderingEngine {
    entry: Rc<Entry>,
    instance: Rc<Instance>,
    physical_device: vk::PhysicalDevice,
    device: Rc<Device>,
    allocator: Option<Rc<vk_mem::Allocator>>,
    surface: Option<vk::SurfaceKHR>,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    queue: vk::Queue,
    swapchain: Option<SwapChain>,
    command_pool: vk::CommandPool,
    debug_callback: Option<vk::DebugUtilsMessengerEXT>,

    imgui: Rc<RefCell<ImguiRenderer>>,
    component_factory: Rc<VulkanComponentFactory>,
    descriptor_manager: Option<Rc<DescriptorManager>>,
    dub_manager: Option<Arc<DynamicUniformBufferManager>>,
    adhoc_command_runner: Rc<AdhocCommandRunner>,

    surface_entry: ash::khr::surface::Instance,
    debug_entry: ash::ext::debug_utils::Instance,

    /// Per-in-flight-frame semaphores signaled by `acquire_next_image`
    /// (consumed by the matching submit's `wait_semaphores`).
    image_available_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    /// Per-in-flight-frame semaphores signaled by the main submit
    /// (consumed by `present`).
    render_finished_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    /// Per-in-flight-frame submit fences. Created in the signaled
    /// state so the very first frame's `wait_for_fences` returns
    /// immediately.
    in_flight_fences: [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    /// Swapchain-image-indexed fence map. Tracks which `in_flight_fence`
    /// (if any) currently owns each swapchain image's resources, so
    /// that when the CPU wraps around and tries to record the same
    /// image again it can wait on the right fence first. Initialized
    /// to `vk::Fence::null()`.
    images_in_flight: Vec<vk::Fence>,
    /// Index of the current in-flight frame (advances mod
    /// `MAX_FRAMES_IN_FLIGHT` after each successful submit).
    current_frame: usize,

    /// Render-finished semaphores from offscreen target submissions that
    /// the next swapchain submit must wait on before sampling. Populated
    /// by `render_scene_to_target`, drained by `render`.
    pending_offscreen_waits: Vec<vk::Semaphore>,
}

impl RenderingEngine for VulkanRenderingEngine {
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame) {
        if self.surface.is_none() {
            self.drain_pending_offscreen();
            return;
        }
        if self.swapchain.is_none() {
            self.drain_pending_offscreen();
            self.recreate_swapchain().unwrap();
            return;
        }

        let entities = scene.visible_entities();

        self.dub_manager().update_do(|updater| {
            for entity in &entities {
                if let Some(rc) = entity.get_rendering_component() {
                    let objects = rc.render_objects();
                    for ro in objects {
                        if let Some(vro) = ro.downcast_ref::<VulkanRenderObject>() {
                            updater(vro.dub_index(), entity.world_transform().matrix());
                        }
                    }
                }
            }
        });

        match self.render_objects(scene.camera().borrow(), entities, viewport, ui_frame) {
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

    fn begin_frame(&mut self) {}

    fn end_frame(&mut self) {}

    fn render_scene_to_target(
        &mut self,
        scene: ComRc<IScene>,
        target: &mut dyn crate::rendering::RenderTarget,
    ) {
        let target = match target.downcast_mut::<super::render_target::VulkanRenderTarget>() {
            Some(t) => t,
            None => {
                log::warn!("render_scene_to_target: target is not a VulkanRenderTarget");
                return;
            }
        };

        let swapchain = match self.swapchain.as_mut() {
            Some(s) => s,
            None => {
                // No swapchain yet means no command pool / pipeline cache
                // wired up; defer until the first `render` call recreates
                // the swapchain. The script-level previewer will retry on
                // the next frame.
                return;
            }
        };

        let entities = scene.visible_entities();

        // Update the dynamic UBO with entity transforms — same bucket as
        // the main pass uses. Safe to share because both pass record the
        // same frame and the engine `wait_idle`s at end-of-frame.
        let dub_manager = self.dub_manager.as_ref().unwrap().clone();
        dub_manager.update_do(|updater| {
            for entity in &entities {
                if let Some(rc) = entity.get_rendering_component() {
                    for ro in rc.render_objects() {
                        if let Some(vro) = ro.downcast_ref::<VulkanRenderObject>() {
                            updater(vro.dub_index(), entity.world_transform().matrix());
                        }
                    }
                }
            }
        });

        // Build opaque / cutout / transparent buckets (mirrors `render_objects`).
        let camera_ref = scene.camera();
        let camera = camera_ref.borrow();
        let camera_world = {
            let m = camera.transform().matrix();
            [m[0][3], m[1][3], m[2][3]]
        };
        let rc: Vec<(Rc<RenderingComponent>, Mat44)> = entities
            .iter()
            .filter_map(|e| {
                let r = e.get_rendering_component()?;
                Some((r, e.world_transform().matrix().clone()))
            })
            .collect();

        let mut opaque: Vec<&VulkanRenderObject> = vec![];
        let mut cutout: Vec<&VulkanRenderObject> = vec![];
        let mut transparent: Vec<(f32, usize, usize, &VulkanRenderObject)> = vec![];
        for (entity_idx, (rendering, world)) in rc.iter().enumerate() {
            let m = world.floats();
            for (ro_idx, ro) in rendering.render_objects().iter().enumerate() {
                if let Some(vro) = ro.downcast_ref::<VulkanRenderObject>() {
                    match vro.material().key().blend {
                        BlendMode::Opaque => opaque.push(vro),
                        BlendMode::AlphaTest => cutout.push(vro),
                        BlendMode::AlphaBlend
                        | BlendMode::Additive
                        | BlendMode::Multiply => {
                            let c = vro.local_centroid();
                            let wx = m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2] + m[0][3];
                            let wy = m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2] + m[1][3];
                            let wz = m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2] + m[2][3];
                            let dx = wx - camera_world[0];
                            let dy = wy - camera_world[1];
                            let dz = wz - camera_world[2];
                            transparent.push((dx * dx + dy * dy + dz * dz, entity_idx, ro_idx, vro));
                        }
                    }
                }
            }
        }
        transparent.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        let transparent_ordered: Vec<&VulkanRenderObject> =
            transparent.into_iter().map(|(_, _, _, ro)| ro).collect();

        // Update target's per-frame UBO with the scene's camera.
        let ubo = {
            let view = Mat44::inversed(camera.transform().matrix());
            let proj = camera.projection_matrix();
            PerFrameUniformBuffer::new(&view, proj)
        };
        target.uniform_buffer_mut().copy_memory_from(&[ubo]);

        // Record + submit the offscreen pass.
        let target_cmd = target.vk_command_buffer();
        let target_rp = target.vk_render_pass();
        let target_fb = target.vk_framebuffer();
        let target_extent = target.vk_extent();
        let target_descriptor = target.per_frame_descriptor_set();
        let target_semaphore = target.vk_render_finished_semaphore();

        if let Err(e) = swapchain.record_to_external_framebuffer(
            target_cmd,
            target_rp,
            target_fb,
            target_extent,
            target_descriptor,
            &opaque,
            &cutout,
            &transparent_ordered,
            &dub_manager,
        ) {
            log::warn!("offscreen record failed: {:?}", e);
            return;
        }

        let signal = [target_semaphore];
        let commands = [target_cmd];
        let submit = vk::SubmitInfo::default()
            .command_buffers(&commands)
            .signal_semaphores(&signal);
        if let Err(e) = self
            .device
            .queue_submit(self.queue, &[submit], vk::Fence::default())
        {
            log::warn!("offscreen submit failed: {:?}", e);
            return;
        }

        self.pending_offscreen_waits.push(target_semaphore);
    }
}

impl VulkanRenderingEngine {
    pub fn new(
        window: &Window,
        imgui_context: &ImguiContext,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Rc::new(Entry::load().unwrap()) };
        let instance = Rc::new(Instance::new(entry.clone()));
        let physical_device = creation_helpers::get_physical_device(&instance.vk_instance())?;

        let surface_entry =
            ash::khr::surface::Instance::new(entry.as_ref(), &instance.vk_instance());
        let surface =
            creation_helpers::create_surface(entry.as_ref(), &instance.vk_instance(), &window)?;

        let graphics_queue_family_index = creation_helpers::get_graphics_queue_family_index(
            &instance.vk_instance(),
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
            let create_info = vk_mem::AllocatorCreateInfo::new(
                instance.vk_instance(),
                device.vk_device(),
                physical_device,
            );

            unsafe { vk_mem::Allocator::new(create_info).unwrap() }
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
            let create_info = vk::CommandPoolCreateInfo::default()
                .flags(
                    vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                        | vk::CommandPoolCreateFlags::TRANSIENT,
                )
                .queue_family_index(graphics_queue_family_index);
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
        let mut swapchain = SwapChain::new(
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
        )
        .unwrap();

        let imgui = Rc::new(RefCell::new(ImguiRenderer::new(
            instance.clone(),
            physical_device,
            device.clone(),
            queue,
            command_pool,
            swapchain.render_pass(),
            descriptor_manager.clone(),
            swapchain.images_len(),
            imgui_context,
        )));

        swapchain.set_imgui(imgui.clone());

        let semaphore_create_info = vk::SemaphoreCreateInfo::default();
        let image_available_semaphores = {
            let mut sems = [vk::Semaphore::null(); MAX_FRAMES_IN_FLIGHT];
            for s in sems.iter_mut() {
                *s = device.create_semaphore(&semaphore_create_info)?;
            }
            sems
        };
        let render_finished_semaphores = {
            let mut sems = [vk::Semaphore::null(); MAX_FRAMES_IN_FLIGHT];
            for s in sems.iter_mut() {
                *s = device.create_semaphore(&semaphore_create_info)?;
            }
            sems
        };
        let in_flight_fences = {
            let mut fences = [vk::Fence::null(); MAX_FRAMES_IN_FLIGHT];
            for f in fences.iter_mut() {
                *f = device.create_fence(true)?;
            }
            fences
        };
        let images_in_flight = vec![vk::Fence::null(); swapchain.images_len()];

        let component_factory = Rc::new(VulkanComponentFactory::new(
            instance.clone(),
            physical_device,
            device.clone(),
            &allocator,
            &descriptor_manager,
            &dub_manager,
            &adhoc_command_runner,
            imgui.clone(),
        ));

        // DEBUG INFO
        let debug_entry = DebugUtils::new(entry.as_ref(), &instance.vk_instance());
        let debug_callback = {
            if !cfg!(target_os = "android") {
                let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
                    s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
                    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                    message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    pfn_user_callback: Some(helpers::debug_callback),
                    flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
                    p_user_data: ptr::null_mut(),
                    p_next: ptr::null(),
                    _marker: std::marker::PhantomData,
                };
                unsafe { Some(debug_entry.create_debug_utils_messenger(&create_info, None)?) }
            } else {
                None
            }
        };

        let vulkan = Self {
            entry,
            instance,
            physical_device,
            device,
            allocator: Some(allocator),
            surface: Some(surface),
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
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            images_in_flight,
            current_frame: 0,
            imgui,
            pending_offscreen_waits: Vec::new(),
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

    pub fn drop_surface(&mut self) {
        self.surface = None;
        self.drop_swapchain();
    }

    pub fn recreate_surface(&mut self, window: &Window) -> Result<(), Box<dyn Error>> {
        self.device.wait_idle();
        if self.surface.is_none() {
            let surface = creation_helpers::create_surface(
                self.entry.as_ref(),
                &self.instance.vk_instance(),
                &window,
            )?;

            self.surface = Some(surface);
            self.recreate_swapchain().unwrap();
        }

        Ok(())
    }

    fn recreate_swapchain(&mut self) -> Result<(), Box<dyn Error>> {
        self.device.wait_idle();

        self.swapchain = None;
        let capabilities = self.get_capabilities()?;
        let mut swapchain = SwapChain::new(
            &self.instance,
            self.device.clone(),
            self.allocator(),
            self.command_pool,
            self.physical_device,
            self.queue,
            self.surface.unwrap(),
            capabilities,
            self.format,
            self.present_mode,
            self.descriptor_manager(),
            &self.adhoc_command_runner,
        )?;
        self.imgui
            .as_ref()
            .borrow_mut()
            .set_render_pass(swapchain.render_pass())?;

        swapchain.set_imgui(self.imgui.clone());

        // Resize the per-image fence map to match the new swapchain's
        // image count. `wait_idle()` above guarantees no in-flight
        // frame is still referencing the previous map, so reseating
        // the vector here is safe even when the count changes.
        self.images_in_flight = vec![vk::Fence::null(); swapchain.images_len()];
        // Reset the in-flight cursor so the next frame starts from a
        // known slot.
        self.current_frame = 0;

        self.swapchain = Some(swapchain);

        Ok(())
    }

    fn render_objects(
        &mut self,
        camera: Ref<Camera>,
        entities: Vec<ComRc<IEntity>>,
        viewport: Viewport,
        ui_frame: ImguiFrame,
    ) -> Result<(), Box<dyn Error>> {
        macro_rules! swapchain {
            ( ) => {
                self.swapchain.as_mut().unwrap()
            };
        }

        let dub_manager = self.dub_manager().clone();

        // Block until this frame's slot is free on the GPU. The fence
        // is created signaled, so frame 0 returns immediately.
        let frame = self.current_frame;
        self.device
            .wait_for_fences(&[self.in_flight_fences[frame]], true, u64::MAX)?;

        let (image_index, _) = match swapchain!().acquire_next_image(
            u64::max_value(),
            self.image_available_semaphores[frame],
            vk::Fence::default(),
        ) {
            Ok(res) => res,
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => {
                self.drop_surface();
                return Ok(());
            }
            Err(e) => panic!("Unable to acquire next image {:?}", e),
        };

        // If a previous in-flight frame is still using this swapchain
        // image's resources (command buffer / uniform buffer indexed
        // by `image_index`), block until that frame finishes before
        // re-recording. This is the per-image guard that lets us drive
        // `MAX_FRAMES_IN_FLIGHT` decoupled from the swapchain image
        // count.
        let prev_fence = self.images_in_flight[image_index as usize];
        if prev_fence != vk::Fence::null() {
            self.device
                .wait_for_fences(&[prev_fence], true, u64::MAX)?;
        }
        self.images_in_flight[image_index as usize] = self.in_flight_fences[frame];

        // Build the visible render-object list, pairing each render
        // object with its owning entity's world matrix and a stable
        // ordinal so transparent sorting can compute per-object centroid
        // distance against the camera (instead of per-entity, which
        // would tie every sub-mesh of a single dff together).
        let camera_world = {
            let m = camera.transform().matrix();
            [m[0][3], m[1][3], m[2][3]]
        };
        let rc: Vec<(Rc<RenderingComponent>, Mat44)> = entities
            .iter()
            .filter_map(|e| {
                let r = e.get_rendering_component()?;
                Some((r, e.world_transform().matrix().clone()))
            })
            .collect();

        let mut opaque: Vec<&VulkanRenderObject> = vec![];
        let mut cutout: Vec<&VulkanRenderObject> = vec![];
        // (dist_sq, entity_idx, ro_idx, ro)
        let mut transparent: Vec<(f32, usize, usize, &VulkanRenderObject)> = vec![];
        for (entity_idx, (rendering, world)) in rc.iter().enumerate() {
            let m = world.floats();
            for (ro_idx, ro) in rendering.render_objects().iter().enumerate() {
                if let Some(vro) = ro.downcast_ref::<VulkanRenderObject>() {
                    match vro.material().key().blend {
                        BlendMode::Opaque => opaque.push(vro),
                        BlendMode::AlphaTest => cutout.push(vro),
                        BlendMode::AlphaBlend
                        | BlendMode::Additive
                        | BlendMode::Multiply => {
                            let c = vro.local_centroid();
                            // Affine transform: world_pos = M * [c, 1].
                            let wx = m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2] + m[0][3];
                            let wy = m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2] + m[1][3];
                            let wz = m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2] + m[2][3];
                            let dx = wx - camera_world[0];
                            let dy = wy - camera_world[1];
                            let dz = wz - camera_world[2];
                            transparent.push((dx * dx + dy * dy + dz * dz, entity_idx, ro_idx, vro));
                        }
                    }
                }
            }
        }
        // Back-to-front: farthest from the camera first. Tie-break by
        // loader-supplied order (entity index, then render-object index)
        // so equal-distance siblings draw in a deterministic, file-defined
        // sequence rather than a hash-randomized one.
        transparent.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        let transparent_ordered: Vec<&VulkanRenderObject> =
            transparent.into_iter().map(|(_, _, _, ro)| ro).collect();

        let command_buffer = swapchain!()
            .record_command_buffers(
                image_index as usize,
                &opaque,
                &cutout,
                &transparent_ordered,
                &dub_manager,
                viewport,
                ui_frame,
            )
            .unwrap();

        // Update Per-frame Uniform Buffers
        {
            let ubo = {
                let view = Mat44::inversed(camera.transform().matrix());
                let proj = camera.projection_matrix();
                PerFrameUniformBuffer::new(&view, proj)
            };

            swapchain!().update_ubo(image_index as usize, &[ubo]);
        }

        // Submit commands
        {
            let commands = [command_buffer];

            // Wait on swapchain image acquisition + any offscreen-target
            // submits queued earlier this frame (their color images need
            // to finish writing before the imgui pass samples them).
            let mut wait_semaphores: Vec<vk::Semaphore> =
                vec![self.image_available_semaphores[frame]];
            let mut stage_mask: Vec<vk::PipelineStageFlags> =
                vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            for sem in self.pending_offscreen_waits.drain(..) {
                wait_semaphores.push(sem);
                stage_mask.push(vk::PipelineStageFlags::FRAGMENT_SHADER);
            }

            let signal_semaphores = [self.render_finished_semaphores[frame]];
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&stage_mask)
                .command_buffers(&commands)
                .signal_semaphores(&signal_semaphores);

            // Reset the fence immediately before re-using it as the
            // submit's completion fence. Doing it here (rather than at
            // the top of the function next to `wait_for_fences`) keeps
            // the fence in the signaled state on every error path that
            // returns before reaching this submit, so we don't deadlock
            // a future frame that waits on an unsignaled fence.
            self.device
                .reset_fences(&[self.in_flight_fences[frame]])?;
            self.device
                .queue_submit(self.queue, &[submit_info], self.in_flight_fences[frame])?;
        }

        // Present
        {
            let wait_semaphores = [self.render_finished_semaphores[frame]];
            let ret = swapchain!().present(image_index, self.queue, &wait_semaphores);

            match ret {
                Ok(false) => (),
                Ok(true) => self.drop_swapchain(),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.drop_swapchain(),
                Err(x) => return Err(Box::new(x) as Box<dyn Error>),
            };
        }

        // Advance to the next in-flight slot. The previous slot's GPU
        // work proceeds asynchronously; we no longer block on it via
        // `device.wait_idle()` — the per-frame fence + per-image guard
        // above is what keeps re-use of command buffers / uniform
        // buffers safe.
        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    fn drop_swapchain(&mut self) {
        self.device.wait_idle();
        self.swapchain = None;
    }

    /// If any offscreen render submits were queued but the swapchain pass
    /// can't run (surface lost, swapchain pending recreate), block on the
    /// queue so those binary semaphores get signaled-then-discarded by an
    /// idle wait rather than left dangling for next frame.
    fn drain_pending_offscreen(&mut self) {
        if !self.pending_offscreen_waits.is_empty() {
            let _ = self.device.queue_wait_idle(self.queue);
            self.pending_offscreen_waits.clear();
        }
    }

    fn get_capabilities(&self) -> ash::prelude::VkResult<vk::SurfaceCapabilitiesKHR> {
        unsafe {
            self.surface_entry.get_physical_device_surface_capabilities(
                self.physical_device,
                self.surface.unwrap(),
            )
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
        if let Some(debug_callback) = self.debug_callback {
            unsafe {
                self.debug_entry
                    .destroy_debug_utils_messenger(debug_callback, None);
            }
        }
        self.device.destroy_command_pool(self.command_pool);

        for s in self.image_available_semaphores.iter() {
            self.device.destroy_semaphore(*s);
        }
        for s in self.render_finished_semaphores.iter() {
            self.device.destroy_semaphore(*s);
        }
        for f in self.in_flight_fences.iter() {
            self.device.destroy_fence(*f);
        }
        unsafe {
            self.surface_entry
                .destroy_surface(self.surface.unwrap(), None);
        }
    }
}
