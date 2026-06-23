use super::creation_helpers;
use super::descriptor_managers::DescriptorManager;
use super::image::Image;
use super::image_view::ImageView;
use super::pipeline_manager::PipelineManager;
use super::render_object::VulkanRenderObject;
use super::render_pass::RenderPass;
use super::uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer};
use super::{adhoc_command_runner::AdhocCommandRunner, device::Device};
use super::{
    buffer::{Buffer, BufferType},
    instance::Instance,
};
use crate::rendering::MaterialKey;
use crate::scene::Viewport;
use crate::{imgui::ImguiFrame, rendering::vulkan::imgui::ImguiRenderer};
use ash::prelude::VkResult;
use ash::vk;
use std::cell::RefCell;
use std::rc::Rc;

/// Optional auxiliary render path that draws the 3D scene at a lower
/// (logical) resolution into an offscreen color image, then blits the
/// result up to the acquired swapchain image (LINEAR filter) and lets
/// imgui render at native swapchain resolution on top.
///
/// Owned by [`SwapChain`] when `SceneScaleMode::Logical` is active.
/// Resources are per swapchain image so they pipeline cleanly with
/// `MAX_FRAMES_IN_FLIGHT` and the per-image fence guard.
struct LogicalRenderPath {
    device: Rc<Device>,
    logical_extent: vk::Extent2D,

    /// Offscreen scene render pass. Final layout for the color
    /// attachment is `TRANSFER_SRC_OPTIMAL` so the immediate next
    /// command can blit it into the swapchain image without a manual
    /// barrier.
    offscreen_render_pass: RenderPass,
    /// Imgui-only render pass that draws on top of the
    /// already-populated swapchain image. Color attachment uses
    /// `LOAD` op (preserve blit result), `initial_layout =
    /// COLOR_ATTACHMENT_OPTIMAL` (caller barriers from
    /// `TRANSFER_DST_OPTIMAL`), `final_layout = PRESENT_SRC_KHR`.
    imgui_render_pass: RenderPass,

    /// Per swapchain image. RAII through `Drop`.
    _offscreen_color_images: Vec<Image>,
    _offscreen_color_views: Vec<ImageView>,
    _offscreen_depth_images: Vec<Image>,
    _offscreen_depth_views: Vec<ImageView>,

    /// Framebuffers for the offscreen scene pass — one per swapchain
    /// image. Sized at `logical_extent`.
    offscreen_framebuffers: Vec<vk::Framebuffer>,
    /// Cached raw color image handles for `vkCmdBlitImage` source.
    offscreen_color_image_handles: Vec<vk::Image>,
    /// Framebuffers for the imgui-only pass — one per swapchain
    /// image. Sized at swapchain `current_extent`.
    imgui_framebuffers: Vec<vk::Framebuffer>,
}

impl LogicalRenderPath {
    fn new(
        instance: &Rc<Instance>,
        device: Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        physical_device: vk::PhysicalDevice,
        swapchain_format: vk::Format,
        swapchain_image_views: &Vec<ImageView>,
        swapchain_extent: vk::Extent2D,
        logical_extent: vk::Extent2D,
        command_runner: &Rc<AdhocCommandRunner>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let logical_extent = vk::Extent2D {
            width: logical_extent.width.max(1),
            height: logical_extent.height.max(1),
        };

        // Probe depth format once via a throw-away depth image (matches
        // the swapchain's existing depth-format probing in `SwapChain::new`).
        // Per-image depths are then created against the same format.
        let probe_depth = Image::new_depth_image(
            &instance.vk_instance(),
            physical_device,
            allocator,
            logical_extent.width,
            logical_extent.height,
        )?;
        let depth_format = probe_depth.vk_format();
        drop(probe_depth);

        let offscreen_render_pass =
            RenderPass::new_offscreen_blit_src(device.clone(), swapchain_format, depth_format);
        let imgui_render_pass = RenderPass::new_imgui_only(device.clone(), swapchain_format);

        let image_count = swapchain_image_views.len();
        let mut offscreen_color_images = Vec::with_capacity(image_count);
        let mut offscreen_color_views = Vec::with_capacity(image_count);
        let mut offscreen_depth_images = Vec::with_capacity(image_count);
        let mut offscreen_depth_views = Vec::with_capacity(image_count);
        let mut offscreen_framebuffers = Vec::with_capacity(image_count);
        let mut offscreen_color_image_handles = Vec::with_capacity(image_count);

        for _ in 0..image_count {
            let mut color = Image::new_color_attachment_image_with(
                allocator,
                logical_extent.width,
                logical_extent.height,
                swapchain_format,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
            )?;
            // Transit once to UNDEFINED→COLOR_ATTACHMENT_OPTIMAL is not
            // needed because the offscreen RP uses `initial_layout =
            // UNDEFINED` (we clear on every frame). Leave the image in
            // its default UNDEFINED state.
            let _ = &mut color;

            let color_view = ImageView::new_color_image_view(
                device.clone(),
                color.vk_image(),
                swapchain_format,
                1,
            )?;

            let mut depth = Image::new_depth_image(
                &instance.vk_instance(),
                physical_device,
                allocator,
                logical_extent.width,
                logical_extent.height,
            )?;
            depth.transit_layout(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                command_runner,
            )?;
            let depth_view = ImageView::new_depth_image_view(
                device.clone(),
                depth.vk_image(),
                depth.vk_format(),
            )?;

            let attachments = [color_view.vk_image_view(), depth_view.vk_image_view()];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(offscreen_render_pass.vk_render_pass())
                .attachments(&attachments)
                .layers(1)
                .width(logical_extent.width)
                .height(logical_extent.height);
            let fb = device.create_framebuffer(&fb_info)?;

            offscreen_color_image_handles.push(color.vk_image());
            offscreen_color_images.push(color);
            offscreen_color_views.push(color_view);
            offscreen_depth_images.push(depth);
            offscreen_depth_views.push(depth_view);
            offscreen_framebuffers.push(fb);
        }

        let imgui_framebuffers = creation_helpers::create_color_only_framebuffers(
            &device,
            swapchain_image_views,
            &swapchain_extent,
            imgui_render_pass.vk_render_pass(),
        )?;

        Ok(Self {
            device,
            logical_extent,
            offscreen_render_pass,
            imgui_render_pass,
            _offscreen_color_images: offscreen_color_images,
            _offscreen_color_views: offscreen_color_views,
            _offscreen_depth_images: offscreen_depth_images,
            _offscreen_depth_views: offscreen_depth_views,
            offscreen_framebuffers,
            offscreen_color_image_handles,
            imgui_framebuffers,
        })
    }
}

impl Drop for LogicalRenderPath {
    fn drop(&mut self) {
        for fb in self.offscreen_framebuffers.drain(..) {
            self.device.destroy_framebuffer(fb);
        }
        for fb in self.imgui_framebuffers.drain(..) {
            self.device.destroy_framebuffer(fb);
        }
    }
}

pub struct SwapChain {
    device: Rc<Device>,
    descriptor_manager: Rc<DescriptorManager>,
    command_pool: vk::CommandPool,
    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    _image_views: Vec<ImageView>,
    _depth_images: Vec<Image>,
    _depth_image_views: Vec<ImageView>,
    uniform_buffers: Vec<Buffer>,
    per_frame_descriptor_sets: Vec<vk::DescriptorSet>,
    framebuffers: Vec<vk::Framebuffer>,
    command_buffers: Vec<vk::CommandBuffer>,
    capabilities: vk::SurfaceCapabilitiesKHR,
    /// Surface format the swapchain was created with. Stored so the
    /// readback path can inspect it without re-querying the surface.
    format: vk::SurfaceFormatKHR,
    pipeline_manager: PipelineManager,
    render_pass: vk::RenderPass,
    imgui: Option<Rc<RefCell<ImguiRenderer>>>,

    /// `Some` when `SceneScaleMode::Logical` is active. Holds the
    /// offscreen scene target + dedicated imgui render pass + auxiliary
    /// framebuffers used by `record_command_buffers`'s logical path.
    /// `None` keeps the original Native path (scene + imgui combined
    /// into one render pass).
    logical: Option<LogicalRenderPath>,

    device_fn: ash::khr::swapchain::Device,
}

impl SwapChain {
    pub fn new(
        instance: &Rc<Instance>,
        device: Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_pool: vk::CommandPool,
        physical_device: vk::PhysicalDevice,
        queue: vk::Queue,
        surface: vk::SurfaceKHR,
        mut capabilities: vk::SurfaceCapabilitiesKHR,
        format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
        descriptor_manager: &Rc<DescriptorManager>,
        command_runner: &Rc<AdhocCommandRunner>,
        logical_extent: Option<vk::Extent2D>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Make it at least 1x1 pixel for images
        capabilities.current_extent.width = capabilities.current_extent.width.max(1);
        capabilities.current_extent.height = capabilities.current_extent.height.max(1);

        let device_fn =
            ash::khr::swapchain::Device::new(&instance.vk_instance(), &device.vk_device());
        // In Logical mode we blit into the swapchain image, which
        // requires TRANSFER_DST usage. Verify the surface supports it
        // before requesting; otherwise fall back to Native to avoid
        // swapchain-creation failure on a restricted platform.
        let want_transfer_dst = logical_extent.is_some();
        let supports_transfer_dst = capabilities
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_DST);
        let use_logical = want_transfer_dst && supports_transfer_dst;
        if want_transfer_dst && !supports_transfer_dst {
            log::warn!(
                "SceneScaleMode::Logical requested but surface does not advertise \
                 TRANSFER_DST usage; falling back to Native rendering."
            );
        }
        let extra_usage = if use_logical {
            vk::ImageUsageFlags::TRANSFER_DST
        } else {
            vk::ImageUsageFlags::empty()
        };
        let handle = creation_helpers::create_swapchain_with_usage(
            &device_fn,
            surface,
            capabilities,
            format,
            present_mode,
            extra_usage,
        )?;

        let images = unsafe { device_fn.get_swapchain_images(handle)? };
        let image_views = creation_helpers::create_image_views(&device, &images, format)?;
        let uniform_buffers: Vec<Buffer> = (0..images.len())
            .map(|_| {
                Buffer::new_dynamic_buffer(
                    allocator,
                    BufferType::Uniform,
                    std::mem::size_of::<PerFrameUniformBuffer>(),
                    1,
                )
                .unwrap()
            })
            .collect();

        // One depth image per swapchain image. The Native render path
        // runs `MAX_FRAMES_IN_FLIGHT` frames concurrently, each targeting
        // a different swapchain color image; sharing a single depth buffer
        // across them races the depth attachment (no cross-frame sync),
        // which shows up as flashing / disappearing geometry on drivers
        // that don't serialize overlapping frames (e.g. AMD). Giving each
        // swapchain image its own depth buffer removes the hazard — the
        // Logical path already does this via `LogicalRenderPath`.
        let mut depth_images = Vec::with_capacity(images.len());
        let mut depth_image_views = Vec::with_capacity(images.len());
        for _ in 0..images.len() {
            let mut depth_image = Image::new_depth_image(
                &instance.vk_instance(),
                physical_device,
                &allocator,
                capabilities.current_extent.width,
                capabilities.current_extent.height,
            )?;

            depth_image.transit_layout(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                command_runner,
            )?;
            let depth_image_view = ImageView::new_depth_image_view(
                device.clone(),
                depth_image.vk_image(),
                depth_image.vk_format(),
            )?;

            depth_images.push(depth_image);
            depth_image_views.push(depth_image_view);
        }
        let depth_format = depth_images[0].vk_format();

        // NOTE: we deliberately do NOT call
        // `descriptor_manager.reset_per_frame_descriptor_pool()` here. The
        // pool is created with `FREE_DESCRIPTOR_SET` and the previous
        // `SwapChain::Drop` already frees its own per-frame descriptor
        // sets explicitly. Resetting would invalidate descriptor sets
        // allocated by `VulkanRenderTarget`s, breaking offscreen previews
        // across swapchain recreation (e.g. window resize).
        let pipeline_manager = PipelineManager::new(
            device.clone(),
            &descriptor_manager,
            format.format,
            depth_format,
            capabilities.current_extent,
        );
        let render_pass = pipeline_manager.render_pass().vk_render_pass();

        let per_frame_descriptor_sets =
            descriptor_manager.allocate_per_frame_descriptor_sets(uniform_buffers.as_slice())?;

        let framebuffers = creation_helpers::create_framebuffers(
            &device,
            &image_views,
            &depth_image_views,
            &capabilities.current_extent,
            render_pass,
        )?;

        let command_buffers = {
            let create_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .command_buffer_count(framebuffers.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY);
            device.allocate_command_buffers(&create_info)?
        };

        let logical = if use_logical {
            let ext = logical_extent.unwrap();
            match LogicalRenderPath::new(
                instance,
                device.clone(),
                allocator,
                physical_device,
                format.format,
                &image_views,
                capabilities.current_extent,
                ext,
                command_runner,
            ) {
                Ok(l) => Some(l),
                Err(e) => {
                    log::error!(
                        "SceneScaleMode::Logical setup failed ({}); falling back to Native.",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            device,
            descriptor_manager: descriptor_manager.clone(),
            command_pool,
            handle,
            images,
            _image_views: image_views,
            _depth_images: depth_images,
            _depth_image_views: depth_image_views,
            uniform_buffers,
            per_frame_descriptor_sets,
            framebuffers,
            command_buffers,
            capabilities,
            format,
            pipeline_manager,
            render_pass,
            imgui: None,
            logical,
            device_fn,
        })
    }

    /// Render pass the imgui renderer should be configured against. In
    /// Native mode this matches the combined scene+ui pass. In Logical
    /// mode this is the dedicated imgui-only render pass that draws on
    /// top of the blitted scene.
    pub fn imgui_render_pass(&self) -> vk::RenderPass {
        match &self.logical {
            Some(l) => l.imgui_render_pass.vk_render_pass(),
            None => self.render_pass,
        }
    }

    /// Logical scene extent in pixels when `SceneScaleMode::Logical` is
    /// active; otherwise the swapchain `current_extent`. Exposed for
    /// diagnostics / future callers.
    #[allow(dead_code)]
    pub fn scene_extent(&self) -> vk::Extent2D {
        match &self.logical {
            Some(l) => l.logical_extent,
            None => self.capabilities.current_extent,
        }
    }

    #[allow(dead_code)]
    pub fn is_logical_mode(&self) -> bool {
        self.logical.is_some()
    }

    pub fn set_imgui(&mut self, imgui: Rc<RefCell<ImguiRenderer>>) {
        self.imgui = Some(imgui);
    }

    pub fn images_len(&self) -> usize {
        self.images.len()
    }

    /// Read-only access to a swapchain image handle. Used by the
    /// rendering engine's `capture_last_frame` to record a transfer
    /// from the most recently presented image into a staging buffer.
    pub fn image(&self, index: usize) -> vk::Image {
        self.images[index]
    }

    /// Surface format the swapchain was created with (carries the
    /// pixel format the readback path needs to interpret).
    pub fn format(&self) -> vk::SurfaceFormatKHR {
        self.format
    }

    /// Current swapchain image extent in pixels. Mirrors the
    /// capabilities snapshot taken at creation; window resizes
    /// recreate the swapchain so this stays accurate for the
    /// lifetime of a single `SwapChain` instance.
    pub fn image_extent(&self) -> vk::Extent2D {
        self.capabilities.current_extent
    }

    pub fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> VkResult<(u32, bool)> {
        unsafe {
            self.device_fn
                .acquire_next_image(self.handle, timeout, semaphore, fence)
        }
    }

    pub fn update_ubo<T>(&mut self, image_index: usize, data: &[T]) {
        self.uniform_buffers[image_index].copy_memory_from(data);
    }

    pub fn present(
        &mut self,
        image_index: u32,
        queue: vk::Queue,
        wait_semaphores: &[vk::Semaphore],
    ) -> VkResult<bool> {
        let swapchains = [self.handle];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe { self.device_fn.queue_present(queue, &present_info) }
    }

    pub fn record_command_buffers(
        &mut self,
        image_index: usize,
        opaque_objects: &[Rc<VulkanRenderObject>],
        cutout_objects: &[Rc<VulkanRenderObject>],
        transparent_objects: &[Rc<VulkanRenderObject>],
        dub_manager: &DynamicUniformBufferManager,
        viewport: Viewport,
        ui_frame: ImguiFrame,
    ) -> Result<vk::CommandBuffer, vk::Result> {
        let command_buffer = self.command_buffers[image_index];
        let per_frame_descriptor_set = self.per_frame_descriptor_sets[image_index];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device.reset_command_buffer(
            command_buffer,
            vk::CommandBufferResetFlags::RELEASE_RESOURCES,
        )?;
        self.device
            .begin_command_buffer(command_buffer, &begin_info)?;

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0f32, 0f32, 0f32, 1f32],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.,
                    stencil: 0,
                },
            },
        ];

        if self.logical.is_some() {
            self.record_logical_path(
                command_buffer,
                image_index,
                per_frame_descriptor_set,
                opaque_objects,
                cutout_objects,
                transparent_objects,
                dub_manager,
                viewport,
                ui_frame,
                &clear_values,
            )?;
        } else {
            let framebuffer = self.framebuffers[image_index];
            let render_pass_begin_info = vk::RenderPassBeginInfo::default()
                .render_pass(self.pipeline_manager.render_pass().vk_render_pass())
                .framebuffer(framebuffer)
                .render_area(
                    vk::Rect2D::default()
                        .offset(vk::Offset2D::default().x(0).y(0))
                        .extent(self.capabilities.current_extent),
                )
                .clear_values(&clear_values);

            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            if let Viewport::CustomViewport(rect) = viewport {
                self.device.cmd_set_viewport(command_buffer, rect);
            } else if let Viewport::FullExtent(rect) = viewport {
                self.device.cmd_set_viewport(command_buffer, rect);
            }

            // Three transparency buckets are submitted in order:
            //   1. Opaque   (blend off, depth-write on) — grouped by MaterialKey
            //      to minimize pipeline switches.
            //   2. Cutout   (AlphaTest: blend off, discard) — same grouping.
            //   3. Transparent (AlphaBlend/Additive/Multiply: depth-write off) —
            //      drawn in the order received (caller back-to-front-sorts);
            //      neighboring objects sharing a material are still grouped.
            //
            // Set 0 (per-frame UBO) is bound exactly once below — it has no
            // dynamic offset and identical layout across every material's
            // pipeline layout, so a single bind is valid for the whole frame.
            // See `draw_groups` for the set 1/2/3 rebind cadence.
            self.bind_per_frame_set_if_any(
                command_buffer,
                per_frame_descriptor_set,
                opaque_objects,
                cutout_objects,
                transparent_objects,
            );
            self.draw_bucket_grouped(command_buffer, opaque_objects, dub_manager);
            self.draw_bucket_grouped(command_buffer, cutout_objects, dub_manager);
            self.draw_bucket_ordered(command_buffer, transparent_objects, dub_manager);

            self.imgui
                .as_ref()
                .unwrap()
                .borrow_mut()
                .record_command_buffer(ui_frame, command_buffer);

            self.device.cmd_end_render_pass(command_buffer);
        }

        self.device.end_command_buffer(command_buffer)?;

        Ok(command_buffer)
    }

    /// Record a command buffer that renders `opaque`/`cutout`/`transparent`
    /// objects into an externally-owned framebuffer (typically a
    /// `VulkanRenderTarget`). No imgui pass; the caller decides the
    /// Logical-mode recording: scene → offscreen (LINEAR-upscale-)
    /// blit → swapchain → imgui on top. Caller has already begun the
    /// command buffer; this method must NOT end it (the outer
    /// `record_command_buffers` calls `end_command_buffer`).
    #[allow(clippy::too_many_arguments)]
    fn record_logical_path(
        &mut self,
        command_buffer: vk::CommandBuffer,
        image_index: usize,
        per_frame_descriptor_set: vk::DescriptorSet,
        opaque_objects: &[Rc<VulkanRenderObject>],
        cutout_objects: &[Rc<VulkanRenderObject>],
        transparent_objects: &[Rc<VulkanRenderObject>],
        dub_manager: &DynamicUniformBufferManager,
        _viewport: Viewport,
        ui_frame: ImguiFrame,
        clear_values: &[vk::ClearValue],
    ) -> Result<(), vk::Result> {
        // SAFETY: caller only enters this branch when `self.logical`
        // is `Some` (checked in `record_command_buffers`).
        let logical = self.logical.as_ref().unwrap();
        let logical_extent = logical.logical_extent;
        let offscreen_fb = logical.offscreen_framebuffers[image_index];
        let offscreen_rp = logical.offscreen_render_pass.vk_render_pass();
        let offscreen_color = logical.offscreen_color_image_handles[image_index];
        let imgui_fb = logical.imgui_framebuffers[image_index];
        let imgui_rp = logical.imgui_render_pass.vk_render_pass();
        let swapchain_image = self.images[image_index];
        let swapchain_extent = self.capabilities.current_extent;

        // ----- 1. Offscreen scene pass -----
        let scene_begin = vk::RenderPassBeginInfo::default()
            .render_pass(offscreen_rp)
            .framebuffer(offscreen_fb)
            .render_area(
                vk::Rect2D::default()
                    .offset(vk::Offset2D::default().x(0).y(0))
                    .extent(logical_extent),
            )
            .clear_values(clear_values);
        self.device.cmd_begin_render_pass(
            command_buffer,
            &scene_begin,
            vk::SubpassContents::INLINE,
        );

        // Viewport always covers the full logical extent. The camera's
        // aspect / projection has already been set by `core_engine` to
        // match `view_extent()`, which returns `logical_extent` in this
        // mode — so we ignore the caller-supplied `Viewport::*` rect
        // (which is itself derived from `view_extent()` and would just
        // produce the same rect).
        self.device.cmd_set_viewport(
            command_buffer,
            crate::math::Rect::new(
                0.0,
                0.0,
                logical_extent.width as f32,
                logical_extent.height as f32,
            ),
        );

        self.bind_per_frame_set_if_any(
            command_buffer,
            per_frame_descriptor_set,
            opaque_objects,
            cutout_objects,
            transparent_objects,
        );
        self.draw_bucket_grouped(command_buffer, opaque_objects, dub_manager);
        self.draw_bucket_grouped(command_buffer, cutout_objects, dub_manager);
        self.draw_bucket_ordered(command_buffer, transparent_objects, dub_manager);

        self.device.cmd_end_render_pass(command_buffer);
        // Offscreen color is now in TRANSFER_SRC_OPTIMAL (RP final
        // layout). No barrier needed before blit.

        // ----- 2. Transition swapchain image UNDEFINED → TRANSFER_DST_OPTIMAL -----
        let color_subresource = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);

        let to_transfer_dst = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(swapchain_image)
            .subresource_range(color_subresource);

        self.device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[to_transfer_dst],
        );

        // ----- 3. Blit offscreen → swapchain (LINEAR upscale) -----
        let src_offsets = [
            vk::Offset3D { x: 0, y: 0, z: 0 },
            vk::Offset3D {
                x: logical_extent.width as i32,
                y: logical_extent.height as i32,
                z: 1,
            },
        ];
        let dst_offsets = [
            vk::Offset3D { x: 0, y: 0, z: 0 },
            vk::Offset3D {
                x: swapchain_extent.width as i32,
                y: swapchain_extent.height as i32,
                z: 1,
            },
        ];
        let blit_subresource = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .mip_level(0)
            .base_array_layer(0)
            .layer_count(1);
        let blit = vk::ImageBlit::default()
            .src_subresource(blit_subresource)
            .dst_subresource(blit_subresource)
            .src_offsets(src_offsets)
            .dst_offsets(dst_offsets);

        unsafe {
            self.device.vk_device().cmd_blit_image(
                command_buffer,
                offscreen_color,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                swapchain_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                vk::Filter::LINEAR,
            );
        }

        // ----- 4. Transition swapchain image TRANSFER_DST → COLOR_ATTACHMENT_OPTIMAL -----
        let to_color = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(swapchain_image)
            .subresource_range(color_subresource);
        self.device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[to_color],
        );

        // ----- 5. Imgui-only pass on swapchain image -----
        let imgui_begin = vk::RenderPassBeginInfo::default()
            .render_pass(imgui_rp)
            .framebuffer(imgui_fb)
            .render_area(
                vk::Rect2D::default()
                    .offset(vk::Offset2D::default().x(0).y(0))
                    .extent(swapchain_extent),
            );
        // No clear values: LOAD op preserves the blit result.
        self.device.cmd_begin_render_pass(
            command_buffer,
            &imgui_begin,
            vk::SubpassContents::INLINE,
        );
        self.imgui
            .as_ref()
            .unwrap()
            .borrow_mut()
            .record_command_buffer(ui_frame, command_buffer);
        self.device.cmd_end_render_pass(command_buffer);

        Ok(())
    }

    /// Record a command buffer that renders `opaque`/`cutout`/`transparent`
    /// objects into an externally-owned framebuffer (typically a
    /// `VulkanRenderTarget`). No imgui pass; the caller decides the
    /// render pass + framebuffer + descriptor set + extent. The pipeline
    /// manager is reused so any new materials get a pipeline created
    /// against the swapchain's render pass (which is render-pass
    /// compatible with the offscreen one).
    #[allow(clippy::too_many_arguments)]
    pub fn record_to_external_framebuffer(
        &mut self,
        command_buffer: vk::CommandBuffer,
        render_pass: vk::RenderPass,
        framebuffer: vk::Framebuffer,
        extent: vk::Extent2D,
        per_frame_descriptor_set: vk::DescriptorSet,
        opaque_objects: &[Rc<VulkanRenderObject>],
        cutout_objects: &[Rc<VulkanRenderObject>],
        transparent_objects: &[Rc<VulkanRenderObject>],
        dub_manager: &DynamicUniformBufferManager,
    ) -> Result<(), vk::Result> {
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device.reset_command_buffer(
            command_buffer,
            vk::CommandBufferResetFlags::RELEASE_RESOURCES,
        )?;
        self.device
            .begin_command_buffer(command_buffer, &begin_info)?;

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0f32, 0f32, 0f32, 1f32],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.,
                    stencil: 0,
                },
            },
        ];
        let render_pass_begin_info = vk::RenderPassBeginInfo::default()
            .render_pass(render_pass)
            .framebuffer(framebuffer)
            .render_area(
                vk::Rect2D::default()
                    .offset(vk::Offset2D::default().x(0).y(0))
                    .extent(extent),
            )
            .clear_values(&clear_values);

        self.device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_begin_info,
            vk::SubpassContents::INLINE,
        );

        // Offscreen viewport spans the full target. Same convention as
        // the swapchain path: positive height, origin at (0,0). Vulkan's
        // Y-down NDC is already accommodated upstream in the projection
        // matrix.
        self.device.cmd_set_viewport(
            command_buffer,
            crate::math::Rect::new(0., 0., extent.width as f32, extent.height as f32),
        );

        self.bind_per_frame_set_if_any(
            command_buffer,
            per_frame_descriptor_set,
            opaque_objects,
            cutout_objects,
            transparent_objects,
        );
        self.draw_bucket_grouped(command_buffer, opaque_objects, dub_manager);
        self.draw_bucket_grouped(command_buffer, cutout_objects, dub_manager);
        self.draw_bucket_ordered(command_buffer, transparent_objects, dub_manager);

        self.device.cmd_end_render_pass(command_buffer);
        self.device.end_command_buffer(command_buffer)?;

        Ok(())
    }

    /// Bind the per-frame descriptor set (set = 0) exactly once for the
    /// upcoming buckets, using the first available material's pipeline
    /// layout. Set 0 has no dynamic offset and every pipeline layout
    /// produced by `DescriptorManager` uses the same layout for set 0,
    /// so this binding stays valid for every subsequent pipeline /
    /// descriptor-set bind in the same command buffer (Vulkan §14.2.2
    /// "Pipeline Layout Compatibility for set 0").
    ///
    /// If every bucket is empty we skip the bind entirely — there's
    /// nothing to draw and no pipeline layout to borrow.
    fn bind_per_frame_set_if_any(
        &mut self,
        command_buffer: vk::CommandBuffer,
        per_frame_descriptor_set: vk::DescriptorSet,
        opaque: &[Rc<VulkanRenderObject>],
        cutout: &[Rc<VulkanRenderObject>],
        transparent: &[Rc<VulkanRenderObject>],
    ) {
        let first = opaque
            .first()
            .or_else(|| cutout.first())
            .or_else(|| transparent.first());
        let Some(obj) = first else {
            return;
        };
        // Make sure the pipeline (and therefore its pipeline_layout)
        // exists before we borrow its layout for the set-0 bind.
        self.pipeline_manager
            .create_pipeline_if_not_exist(obj.material());
        let pipeline = self.pipeline_manager.get_pipeline(obj.material().key());
        let layout = pipeline.pipeline_layout().vk_pipeline_layout();
        self.device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            layout,
            0,
            &[per_frame_descriptor_set],
            &[],
        );
    }

    fn draw_bucket_grouped(
        &mut self,
        command_buffer: vk::CommandBuffer,
        objects: &[Rc<VulkanRenderObject>],
        dub_manager: &DynamicUniformBufferManager,
    ) {
        let mut material_index: std::collections::HashMap<MaterialKey, usize> =
            std::collections::HashMap::new();
        let mut materials: Vec<*const super::material::VulkanMaterial> = vec![];
        let mut groups: Vec<Vec<Rc<VulkanRenderObject>>> = vec![];
        for obj in objects {
            let key = *obj.material().key();
            if let Some(&idx) = material_index.get(&key) {
                groups[idx].push(obj.clone());
            } else {
                material_index.insert(key, materials.len());
                self.pipeline_manager
                    .create_pipeline_if_not_exist(obj.material());
                materials.push(obj.material() as *const _);
                groups.push(vec![obj.clone()]);
            }
        }
        self.draw_groups(command_buffer, &materials, &groups, dub_manager);
    }

    fn draw_bucket_ordered(
        &mut self,
        command_buffer: vk::CommandBuffer,
        objects: &[Rc<VulkanRenderObject>],
        dub_manager: &DynamicUniformBufferManager,
    ) {
        // Preserve caller-supplied order; only consolidate consecutive
        // objects that share a MaterialKey. Required for back-to-front
        // correctness on translucent geometry.
        let mut materials: Vec<*const super::material::VulkanMaterial> = vec![];
        let mut groups: Vec<Vec<Rc<VulkanRenderObject>>> = vec![];
        let mut last_key: Option<MaterialKey> = None;
        for obj in objects {
            let key = *obj.material().key();
            if Some(key) != last_key {
                self.pipeline_manager
                    .create_pipeline_if_not_exist(obj.material());
                materials.push(obj.material() as *const _);
                groups.push(vec![obj.clone()]);
                last_key = Some(key);
            } else {
                groups.last_mut().unwrap().push(obj.clone());
            }
        }
        self.draw_groups(command_buffer, &materials, &groups, dub_manager);
    }

    /// Inner-loop draw helper. Assumes set 0 (per-frame) has already
    /// been bound by `bind_per_frame_set_if_any` and remains valid for
    /// the whole render pass.
    ///
    /// Rebind cadence:
    ///   - **Pipeline**: once per group (each group shares one
    ///     `MaterialKey` ⇒ one pipeline).
    ///   - **Set 3** (per-material UBO): only when the
    ///     `VulkanMaterial` pointer changes inside a group. Multiple
    ///     distinct materials can share the same `MaterialKey`
    ///     (PAL4 water UV animation forces unique materials per
    ///     animated object via `make_unique`), so we re-bind set 3
    ///     against this object's own material.
    ///   - **Sets 1 + 2** (dub + per-object): bound together every
    ///     draw with `firstSet = 1`, `descriptorSetCount = 2` and the
    ///     dub's dynamic offset. Set 1's descriptor type is
    ///     `UNIFORM_BUFFER_DYNAMIC` so the offset must be supplied
    ///     per draw.
    ///   - **Vertex / index buffers**: skipped when the previous
    ///     object used the same `vk::Buffer` handle.
    fn draw_groups(
        &mut self,
        command_buffer: vk::CommandBuffer,
        materials: &[*const super::material::VulkanMaterial],
        groups: &[Vec<Rc<VulkanRenderObject>>],
        dub_manager: &DynamicUniformBufferManager,
    ) {
        let mut last_material_ptr: *const super::material::VulkanMaterial = std::ptr::null();
        let mut last_vertex_buffer: vk::Buffer = vk::Buffer::null();
        let mut last_index_buffer: vk::Buffer = vk::Buffer::null();

        for (index, object_group) in groups.iter().enumerate() {
            // SAFETY: each pointer originates from `obj.material()` whose
            // lifetime is bound to `objects` (still on the stack of the
            // caller's `record_command_buffers` invocation).
            let group_material = unsafe { &*materials[index] };
            let pipeline = self.pipeline_manager.get_pipeline(group_material.key());
            let pipeline_layout = pipeline.pipeline_layout().vk_pipeline_layout();

            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.vk_pipeline(),
            );

            // A new pipeline can disturb sets 2/3 bound under a previous
            // pipeline layout if its set-2 layout differs. We always
            // rebind set 2 + set 3 below for the first object of the
            // group, so just clear the trackers.
            last_material_ptr = std::ptr::null();

            for object in object_group {
                let vertex_buffer = object.vertex_buffer();
                let index_buffer = object.index_buffer();
                let vb = vertex_buffer.vk_buffer();
                let ib = index_buffer.vk_buffer();
                if vb != last_vertex_buffer {
                    self.device
                        .cmd_bind_vertex_buffers(command_buffer, 0, &[vb], &[0]);
                    last_vertex_buffer = vb;
                }
                if ib != last_index_buffer {
                    self.device
                        .cmd_bind_index_buffer(command_buffer, ib, 0, vk::IndexType::UINT32);
                    last_index_buffer = ib;
                }

                // `MaterialKey`-grouping above only consolidates pipeline
                // state (program/blend/depth/cull). Multiple distinct
                // materials with the same key share a group — they share
                // the pipeline but each still owns its own per-material
                // UBO (tint / alpha_ref / uv_xform). Read the per-material
                // descriptor from THIS object's material, not the group's
                // representative, otherwise runtime per-material mutations
                // (e.g. PAL4 water UV animation) silently leak onto every
                // other render object sharing the key — typically every
                // textured opaque mesh in the scene.
                let object_material = object.material();
                let object_material_ptr = object_material as *const super::material::VulkanMaterial;
                if object_material_ptr != last_material_ptr {
                    self.device.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_layout,
                        3,
                        &[object_material.material_params_descriptor_set()],
                        &[],
                    );
                    last_material_ptr = object_material_ptr;
                }

                self.device.cmd_bind_descriptor_sets(
                    command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    1,
                    &[dub_manager.descriptor_set(), object.vk_descriptor_set()],
                    &[dub_manager.get_offset(object.dub_index()) as u32],
                );
                self.device.cmd_draw_indexed(
                    command_buffer,
                    index_buffer.element_count(),
                    1,
                    0,
                    0,
                    0,
                );
            }
        }

        // Suppress unused warning when no draws happened.
        let _ = last_material_ptr;
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        for buffer in &self.framebuffers {
            self.device.destroy_framebuffer(*buffer);
        }

        self.descriptor_manager
            .free_per_frame_descriptor_sets(&self.per_frame_descriptor_sets);

        self.device
            .free_command_buffers(self.command_pool, &self.command_buffers);
        unsafe {
            self.device_fn.destroy_swapchain(self.handle, None);
        }
    }
}
