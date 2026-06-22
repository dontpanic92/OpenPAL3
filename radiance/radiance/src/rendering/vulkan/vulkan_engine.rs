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
use crate::comdef::{IEntity, IEntityExt, IScene, ISceneExt};
use crate::math::Mat44;
use crate::rendering::{BlendMode, RenderObject, RenderingComponent};
use crate::scene::{Camera, Viewport};
use crate::{
    imgui::{ImguiContext, ImguiFrame},
    rendering::{ComponentFactory, RenderingEngine, Window},
};
use ash::ext::debug_utils::Instance as DebugUtils;
use ash::{Entry, vk};
use crosscom::ComRc;

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

    /// Logical scene-pass extent in pixels when
    /// `SceneScaleMode::Logical` is configured at construction.
    /// `None` for Native mode (the default). Stored so swapchain
    /// recreation reuses the same value without re-querying the host
    /// window.
    logical_extent: Option<vk::Extent2D>,

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

    /// Swapchain image index that was most recently `present`ed. `None`
    /// until the first successful present — read by `capture_last_frame`
    /// to know which swapchain image holds the user-visible pixels. Reset
    /// on swapchain drop (the new swapchain's image indexing is fresh).
    last_presented_image_index: Option<u32>,

    // Per-frame scratch storage. Hoisted onto the engine so each pass
    // reuses the prior frame's capacity instead of allocating a fresh
    // `Vec` for every extraction — addresses the per-frame allocator
    // churn flagged in `generated/engine_analysis.md` ("Render extraction
    // allocates and downcasts every frame"). Each pass calls `clear()`
    // on these at entry; the `Rc<VulkanRenderObject>` clones are cheap
    // (single non-atomic refcount bump) and avoid the storage-borrow
    // lifetime problem that `&VulkanRenderObject` would create.
    scratch_components: Vec<(Rc<RenderingComponent>, Mat44)>,
    scratch_opaque: Vec<Rc<VulkanRenderObject>>,
    scratch_cutout: Vec<Rc<VulkanRenderObject>>,
    scratch_transparent: Vec<(f32, usize, usize, Rc<VulkanRenderObject>)>,
    scratch_transparent_ordered: Vec<Rc<VulkanRenderObject>>,
}

impl RenderingEngine for VulkanRenderingEngine {
    fn render(&mut self, scene: Option<ComRc<IScene>>, viewport: Viewport, ui_frame: ImguiFrame) {
        if self.surface.is_none() {
            self.drain_pending_offscreen();
            return;
        }
        if self.swapchain.is_none() {
            self.drain_pending_offscreen();
            self.recreate_swapchain().unwrap();
            return;
        }

        // No scene on the stack: still record + present so imgui from
        // pure-script directors reaches the swapchain. We pass an empty
        // entity list and a default camera; record_command_buffers handles
        // the empty 3D pass (just clears) and the imgui pass is independent.
        let (entities, camera) = match scene.as_ref() {
            Some(s) => {
                let entities =
                    crate::perf::time("vulkan.render.collect_visible_entities_total_ns", || {
                        s.visible_entities()
                    });
                (entities, Some(s.camera()))
            }
            None => (Vec::new(), None),
        };
        let lighting = match scene.as_ref() {
            Some(s) => Self::collect_scene_lights(s),
            None => ([0.0; 3], Vec::new()),
        };
        crate::perf::gauge("vulkan.render.visible_entities", entities.len() as u64);

        // Update the dynamic UBO with each visible render object's world
        // transform. Iterates the backend-typed view on RenderingComponent,
        // so no per-RO downcast is needed.
        let updated_vros = std::cell::Cell::new(0u64);
        crate::perf::time("vulkan.render.ubo_update_total_ns", || {
            self.dub_manager().update_do(|updater| {
                for entity in &entities {
                    if let Some(rc) = entity.get_rendering_component() {
                        for vro in rc.vulkan_render_objects() {
                            updater(vro.dub_index(), entity.world_transform().matrix());
                            updated_vros.set(updated_vros.get() + 1);
                        }
                    }
                }
            });
        });
        crate::perf::gauge("vulkan.render.updated_vros", updated_vros.get());
        crate::perf::gauge(
            "vulkan.dub.allocated_slots",
            self.dub_manager().allocated_slots() as u64,
        );

        let result = crate::perf::time("vulkan.render.objects_total_ns", || {
            self.render_objects(camera.as_ref().map(|c| &**c), entities, lighting, viewport, ui_frame)
        });
        if let Err(err) = result {
            println!("{}", err);
        }
    }

    fn view_extent(&self) -> (u32, u32) {
        // In Logical mode the camera viewport / aspect must derive
        // from the offscreen scene target, not the (HiDPI) swapchain.
        if let Some(ext) = self.logical_extent.as_ref() {
            return (ext.width, ext.height);
        }
        (
            self.get_capabilities().unwrap().current_extent.width,
            self.get_capabilities().unwrap().current_extent.height,
        )
    }

    fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        self.component_factory.as_component_factory()
    }

    fn notify_resized(&mut self, logical_size: (u32, u32)) {
        // Nothing to rebuild without a surface, and ignore zero-size
        // (minimized) windows — the next non-zero resize will retrigger
        // us, and recreating a zero-extent target would just fail.
        if self.surface.is_none() || logical_size.0 == 0 || logical_size.1 == 0 {
            return;
        }

        // In Logical mode the scene is rendered to a fixed offscreen image
        // and upscaled to the swapchain. That offscreen must follow the
        // window's *logical* extent so the upscale stays a pure
        // (aspect-preserving) DPI scale; leaving it at the boot-time size
        // stretches the scene when the window's aspect ratio changes.
        // Native mode keeps `logical_extent == None` and renders directly
        // at the swapchain extent, so there is nothing to re-track.
        if self.logical_extent.is_some() {
            self.logical_extent = Some(vk::Extent2D {
                width: logical_size.0,
                height: logical_size.1,
            });
        }

        // Drop the swapchain so the next `render` rebuilds it (and, in
        // Logical mode, the offscreen targets) at the new extent. Doing it
        // here applies the updated `logical_extent` deterministically
        // rather than waiting for the present path to report a suboptimal
        // swapchain.
        self.drop_swapchain();
    }

    fn begin_frame(&mut self) {}

    fn end_frame(&mut self) {}

    fn render_scene_to_target(
        &mut self,
        scene: ComRc<IScene>,
        target: &mut dyn crate::rendering::RenderTarget,
    ) {
        let target = match target.as_vulkan_mut() {
            Some(t) => t,
            None => {
                log::warn!("render_scene_to_target: target is not a VulkanRenderTarget");
                return;
            }
        };

        if self.swapchain.is_none() {
            // No swapchain yet means no command pool / pipeline cache
            // wired up; defer until the first `render` call recreates
            // the swapchain. The script-level previewer will retry on
            // the next frame.
            return;
        }

        let entities = scene.visible_entities();

        // Update the dynamic UBO with entity transforms — same bucket as
        // the main pass uses. Both passes read this within the same frame
        // (content is written before either records). Cross-frame reuse of
        // the offscreen command buffer / UBOs is guarded by the target's
        // own in-flight fence (waited on below before re-recording), so the
        // engine no longer needs an end-of-frame `wait_idle` to stay safe.
        let dub_manager = self.dub_manager.as_ref().unwrap().clone();
        dub_manager.update_do(|updater| {
            for entity in &entities {
                if let Some(rc) = entity.get_rendering_component() {
                    for vro in rc.vulkan_render_objects() {
                        updater(vro.dub_index(), entity.world_transform().matrix());
                    }
                }
            }
        });

        // Build opaque / cutout / transparent buckets (mirrors `render_objects`).
        let (camera_view, camera_proj) = {
            let camera = scene.camera();
            (
                Mat44::inversed(camera.transform().matrix()),
                *camera.projection_matrix(),
            )
        };

        Self::bucketize_visible(
            &entities,
            &scene.camera(),
            &mut self.scratch_components,
            &mut self.scratch_opaque,
            &mut self.scratch_cutout,
            &mut self.scratch_transparent,
            &mut self.scratch_transparent_ordered,
        );

        // Update target's per-frame UBO with the scene's camera + lights.
        let mut ubo = PerFrameUniformBuffer::new(&camera_view, &camera_proj);
        let (ambient, gpu_lights) = Self::collect_scene_lights(&scene);
        ubo.set_lighting(ambient, &gpu_lights);
        target.uniform_buffer_mut().copy_memory_from(&[ubo]);

        // Record + submit the offscreen pass.
        let target_cmd = target.vk_command_buffer();
        let target_rp = target.vk_render_pass();
        let target_fb = target.vk_framebuffer();
        let target_extent = target.vk_extent();
        let target_descriptor = target.per_frame_descriptor_set();
        let target_semaphore = target.vk_render_finished_semaphore();
        let target_fence = target.vk_in_flight_fence();

        // Block until the previous offscreen submit using this target's
        // single command buffer has completed before resetting/re-recording
        // it. Without this, a heavy scene (e.g. a large BSP) lets the GPU
        // fall behind and the host overwrites an in-flight command buffer,
        // which loses the device (ERROR_DEVICE_LOST).
        if let Err(e) = self.device.wait_for_fences(&[target_fence], true, u64::MAX) {
            log::warn!("offscreen fence wait failed: {:?}", e);
            return;
        }

        let swapchain = self.swapchain.as_mut().unwrap();
        if let Err(e) = swapchain.record_to_external_framebuffer(
            target_cmd,
            target_rp,
            target_fb,
            target_extent,
            target_descriptor,
            &self.scratch_opaque,
            &self.scratch_cutout,
            &self.scratch_transparent_ordered,
            &dub_manager,
        ) {
            // Fence is still signaled (we never reset it), so the next
            // frame's `wait_for_fences` won't deadlock.
            log::warn!("offscreen record failed: {:?}", e);
            return;
        }

        let signal = [target_semaphore];
        let commands = [target_cmd];
        let submit = vk::SubmitInfo::default()
            .command_buffers(&commands)
            .signal_semaphores(&signal);
        // Reset the fence only now that we're certain to submit, so every
        // early-return path above leaves it signaled.
        if let Err(e) = self.device.reset_fences(&[target_fence]) {
            log::warn!("offscreen fence reset failed: {:?}", e);
            return;
        }
        if let Err(e) = self
            .device
            .queue_submit(self.queue, &[submit], target_fence)
        {
            log::warn!("offscreen submit failed: {:?}", e);
            return;
        }

        self.pending_offscreen_waits.push(target_semaphore);
    }

    fn update_imgui_font_atlas(&mut self, context: &crate::imgui::ImguiContext) {
        let queue = self.queue;
        let command_pool = self.command_pool;
        self.imgui
            .borrow_mut()
            .rebuild_font_atlas(queue, command_pool, context);
    }

    fn capture_last_frame(&mut self) -> Option<crate::rendering::CapturedFrame> {
        // Pre-flight: bail without device wait when there's nothing to
        // read back. Headless boots and post-`drop_swapchain` states
        // hit this fast-path and return `None`.
        let image_index = self.last_presented_image_index?;
        let swapchain = self.swapchain.as_ref()?;

        let extent = swapchain.image_extent();
        let format = swapchain.format();
        let image = swapchain.image(image_index as usize);

        if extent.width == 0 || extent.height == 0 {
            log::debug!("capture_last_frame: zero-extent swapchain image");
            return None;
        }

        // Determine pixel layout. We only support the two 8-bit color
        // formats actually picked by `creation_helpers::get_format` on
        // every platform we ship — anything exotic returns `None` with
        // a debug log so callers can surface a 501.
        let bgra_needs_swap = match format.format {
            vk::Format::R8G8B8A8_UNORM | vk::Format::R8G8B8A8_SRGB => false,
            vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB => true,
            other => {
                log::debug!(
                    "capture_last_frame: unsupported swapchain format {:?}",
                    other
                );
                return None;
            }
        };

        let width = extent.width;
        let height = extent.height;
        let byte_count = (width as usize) * (height as usize) * 4;

        // Allocate the host-visible staging buffer. `Uniform` would be
        // semantically wrong, so call the low-level constructor with
        // `TRANSFER_DST` usage directly. `MemoryUsage::AutoPreferHost`
        // gives us a mapped + host-coherent allocation.
        let allocator = self.allocator.as_ref()?.clone();
        let mut staging = match super::buffer::Buffer::new_staging_dst(&allocator, byte_count) {
            Ok(b) => b,
            Err(e) => {
                log::error!("capture_last_frame: staging buffer alloc failed: {e}");
                return None;
            }
        };

        // Block until every in-flight frame is done so the swapchain
        // image is safe to read. Coarse, but fine for an off-the-hot-
        // path one-shot like this.
        self.device.wait_idle();

        // Record + submit the readback in a single one-shot command
        // buffer: PRESENT_SRC_KHR → TRANSFER_SRC_OPTIMAL → copy →
        // PRESENT_SRC_KHR.
        let copy_extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };
        let buffer_handle = staging.vk_buffer();

        let submit_result = self
            .adhoc_command_runner
            .run_commands_one_shot(|device, &cb| {
                let subresource = vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);

                let to_transfer_src = vk::ImageMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::MEMORY_READ)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .old_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(subresource);

                device.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[to_transfer_src],
                );

                let copy_region = vk::BufferImageCopy::default()
                    .buffer_offset(0)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                    .image_extent(copy_extent);

                unsafe {
                    device.vk_device().cmd_copy_image_to_buffer(
                        cb,
                        image,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        buffer_handle,
                        &[copy_region],
                    );
                }

                let back_to_present = vk::ImageMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(subresource);

                device.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[back_to_present],
                );
            });

        if let Err(e) = submit_result {
            log::error!("capture_last_frame: command submit failed: {e}");
            return None;
        }

        // The one-shot runner queue-wait-idles before returning, so
        // the staging buffer is safe to read here.
        let mut rgba = staging.read_into_vec(byte_count);

        if bgra_needs_swap {
            // BGRA → RGBA in place. Done in-CPU instead of via a
            // shader because the cost is negligible for one-off
            // screenshots and avoids touching pipeline cache.
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }

        Some(crate::rendering::CapturedFrame {
            width,
            height,
            rgba,
        })
    }
}

impl VulkanRenderingEngine {
    pub fn new(
        window: &Window,
        imgui_context: &ImguiContext,
        options: crate::rendering::RenderingEngineOptions,
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
        let logical_extent = match options.scene_scale_mode {
            crate::rendering::SceneScaleMode::Logical => {
                options.logical_extent.map(|(w, h)| vk::Extent2D {
                    width: w.max(1),
                    height: h.max(1),
                })
            }
            crate::rendering::SceneScaleMode::Native => None,
        };
        if matches!(
            options.scene_scale_mode,
            crate::rendering::SceneScaleMode::Logical
        ) && logical_extent.is_none()
        {
            log::warn!(
                "SceneScaleMode::Logical requested but no logical_extent supplied; \
                 falling back to Native."
            );
        }
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
            logical_extent,
        )
        .unwrap();

        let imgui = Rc::new(RefCell::new(ImguiRenderer::new(
            instance.clone(),
            physical_device,
            device.clone(),
            queue,
            command_pool,
            swapchain.imgui_render_pass(),
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
            last_presented_image_index: None,
            scratch_components: Vec::new(),
            scratch_opaque: Vec::new(),
            scratch_cutout: Vec::new(),
            scratch_transparent: Vec::new(),
            scratch_transparent_ordered: Vec::new(),
            logical_extent,
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
            self.logical_extent,
        )?;
        self.imgui
            .as_ref()
            .borrow_mut()
            .set_render_pass(swapchain.imgui_render_pass())?;

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

    /// Snapshot the scene's lighting environment into the per-frame UBO's
    /// expected shape: a flat ambient color and a list of `(position, color)`
    /// point lights.
    fn collect_scene_lights(
        scene: &ComRc<IScene>,
    ) -> ([f32; 3], Vec<([f32; 3], [f32; 3])>) {
        let lighting = scene.lighting();
        let lights = lighting
            .lights
            .iter()
            .map(|l| ([l.position.x, l.position.y, l.position.z], l.color))
            .collect();
        (lighting.ambient, lights)
    }

    fn render_objects(
        &mut self,
        camera: Option<&Camera>,
        entities: Vec<ComRc<IEntity>>,
        lighting: ([f32; 3], Vec<([f32; 3], [f32; 3])>),
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
            self.device.wait_for_fences(&[prev_fence], true, u64::MAX)?;
        }
        self.images_in_flight[image_index as usize] = self.in_flight_fences[frame];

        // Build the visible render-object list, pairing each render
        // object with its owning entity's world matrix and a stable
        // ordinal so transparent sorting can compute per-object centroid
        // distance against the camera (instead of per-entity, which
        // would tie every sub-mesh of a single dff together).
        //
        // When there's no scene (camera is None) entities is empty, so
        // skip bucketization entirely and clear the scratch buckets so
        // the empty 3D pass just clears the framebuffer.
        if let Some(cam) = camera {
            Self::bucketize_visible(
                &entities,
                cam,
                &mut self.scratch_components,
                &mut self.scratch_opaque,
                &mut self.scratch_cutout,
                &mut self.scratch_transparent,
                &mut self.scratch_transparent_ordered,
            );
        } else {
            self.scratch_components.clear();
            self.scratch_opaque.clear();
            self.scratch_cutout.clear();
            self.scratch_transparent.clear();
            self.scratch_transparent_ordered.clear();
        }

        let command_buffer = swapchain!()
            .record_command_buffers(
                image_index as usize,
                &self.scratch_opaque,
                &self.scratch_cutout,
                &self.scratch_transparent_ordered,
                &dub_manager,
                viewport,
                ui_frame,
            )
            .unwrap();

        // Update Per-frame Uniform Buffers. With no camera (scene-less
        // imgui frame), substitute identity matrices — the empty
        // 3D pass doesn't sample them anyway, but the UBO slot still
        // needs valid data so descriptor binding doesn't trip
        // validation.
        {
            let ubo = match camera {
                Some(cam) => {
                    let view = Mat44::inversed(cam.transform().matrix());
                    let proj = cam.projection_matrix();
                    let mut ubo = PerFrameUniformBuffer::new(&view, proj);
                    ubo.set_lighting(lighting.0, &lighting.1);
                    ubo
                }
                None => {
                    let identity = Mat44::new_identity();
                    PerFrameUniformBuffer::new(&identity, &identity)
                }
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
            self.device.reset_fences(&[self.in_flight_fences[frame]])?;
            self.device
                .queue_submit(self.queue, &[submit_info], self.in_flight_fences[frame])?;
        }

        // Present
        {
            let wait_semaphores = [self.render_finished_semaphores[frame]];
            let ret = swapchain!().present(image_index, self.queue, &wait_semaphores);

            match ret {
                Ok(false) => {
                    // Track the just-presented image so capture_last_frame
                    // can read it back. We deliberately don't update on
                    // suboptimal (Ok(true)) / out-of-date errors below
                    // because the swapchain is about to be dropped.
                    self.last_presented_image_index = Some(image_index);
                }
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
        // Indices into the old swapchain's image vec are meaningless
        // for the next one; clear so capture_last_frame doesn't try to
        // read a stale slot.
        self.last_presented_image_index = None;
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

    /// Walk the visible entity list once, collecting rendering components +
    /// world matrices into `components_out`, then filter each
    /// `VulkanRenderObject` through frustum culling and bucket the
    /// survivors into `opaque_out` / `cutout_out` / `transparent_out`
    /// by `BlendMode`. The transparent bucket is sorted back-to-front
    /// using camera distance + a stable tie-break, then projected into
    /// `transparent_ordered_out` for the renderer.
    ///
    /// Every output Vec is `clear()`ed (capacity retained) before
    /// population, so callers should pass the engine's persistent
    /// scratch fields to avoid per-frame allocations.
    fn bucketize_visible(
        entities: &[ComRc<IEntity>],
        camera: &Camera,
        components_out: &mut Vec<(Rc<RenderingComponent>, Mat44)>,
        opaque_out: &mut Vec<Rc<VulkanRenderObject>>,
        cutout_out: &mut Vec<Rc<VulkanRenderObject>>,
        transparent_out: &mut Vec<(f32, usize, usize, Rc<VulkanRenderObject>)>,
        transparent_ordered_out: &mut Vec<Rc<VulkanRenderObject>>,
    ) {
        components_out.clear();
        opaque_out.clear();
        cutout_out.clear();
        transparent_out.clear();
        transparent_ordered_out.clear();

        for entity in entities {
            if let Some(rc) = entity.get_rendering_component() {
                components_out.push((rc, entity.world_transform().matrix().clone()));
            }
        }

        let camera_world = {
            let m = camera.transform().matrix();
            [m[0][3], m[1][3], m[2][3]]
        };
        let frustum = camera.frustum();

        let mut culled_count: u64 = 0;
        let mut total_count: u64 = 0;
        for (entity_idx, (rendering, world)) in components_out.iter().enumerate() {
            let m = world.floats();
            for (ro_idx, vro) in rendering.vulkan_render_objects().iter().enumerate() {
                total_count += 1;
                // Frustum reject. Render objects without a known
                // local AABB (e.g. backends or callers that don't
                // track bounds) fall through to "always visible".
                if let Some((lmin, lmax)) = vro.local_aabb() {
                    let (wmin, wmax) = crate::math::transform_aabb(lmin, lmax, world);
                    if !crate::math::aabb_visible(wmin, wmax, &frustum) {
                        culled_count += 1;
                        continue;
                    }
                }
                match vro.material().key().blend {
                    BlendMode::Opaque => opaque_out.push(vro.clone()),
                    BlendMode::AlphaTest => cutout_out.push(vro.clone()),
                    BlendMode::AlphaBlend | BlendMode::Additive | BlendMode::Multiply => {
                        let c = vro.local_centroid();
                        // Affine transform: world_pos = M * [c, 1].
                        let wx = m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2] + m[0][3];
                        let wy = m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2] + m[1][3];
                        let wz = m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2] + m[2][3];
                        let dx = wx - camera_world[0];
                        let dy = wy - camera_world[1];
                        let dz = wz - camera_world[2];
                        transparent_out.push((
                            dx * dx + dy * dy + dz * dz,
                            entity_idx,
                            ro_idx,
                            vro.clone(),
                        ));
                    }
                }
            }
        }
        crate::perf::gauge("vulkan.render.total_vros", total_count);
        crate::perf::gauge("vulkan.render.culled_vros", culled_count);
        crate::perf::gauge("vulkan.render.opaque_draws", opaque_out.len() as u64);
        crate::perf::gauge("vulkan.render.cutout_draws", cutout_out.len() as u64);
        crate::perf::gauge(
            "vulkan.render.transparent_draws",
            transparent_out.len() as u64,
        );
        #[cfg(debug_assertions)]
        log::trace!(
            "frustum cull: drew {} / {} render objects ({} culled)",
            total_count - culled_count,
            total_count,
            culled_count,
        );

        // Back-to-front: farthest from the camera first. Tie-break by
        // loader-supplied order (entity index, then render-object index)
        // so equal-distance siblings draw in a deterministic, file-defined
        // sequence rather than a hash-randomized one.
        transparent_out.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        transparent_ordered_out.extend(transparent_out.iter().map(|(_, _, _, ro)| ro.clone()));
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
