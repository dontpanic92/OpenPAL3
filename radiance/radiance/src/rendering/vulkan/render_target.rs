//! Vulkan offscreen render target.
//!
//! Owns a self-contained color/depth framebuffer plus the per-frame
//! resources needed to drive a render pass that does not touch the
//! swapchain. Registers its color image with `imgui-rs-vulkan-renderer`
//! so the editor can sample it via `ui.image(...)` inside a tab body.
//!
//! Pipelines created against the swapchain's `PipelineManager` are reused
//! here because the offscreen render pass is *render-pass compatible*
//! with the swapchain one (same color/depth formats, same subpass
//! structure — only final layouts differ, which doesn't affect
//! compatibility per the Vulkan spec).

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;

use ash::vk;
use imgui::TextureId;

use crate::rendering::RenderTarget;

use super::adhoc_command_runner::AdhocCommandRunner;
use super::buffer::{Buffer, BufferType};
use super::descriptor_managers::DescriptorManager;
use super::device::Device;
use super::image::Image;
use super::image_view::ImageView;
use super::imgui::ImguiRenderer;
use super::instance::Instance;
use super::render_pass::RenderPass;
use super::uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer};

/// Color attachment format used for offscreen targets. Matches what
/// `Image::new_color_attachment_image` produces and what the swapchain's
/// `PipelineManager` is configured with (via `swapchain.format.format`),
/// so render-pass compatibility — and pipeline reuse — is guaranteed.
const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

/// Resources that get recreated on every `resize`. Kept in their own
/// struct so we can swap them out atomically and so `Drop` doesn't need
/// to special-case half-initialized state.
struct Attachments {
    _color_image: Image,
    _color_view: ImageView,
    _depth_image: Image,
    _depth_view: ImageView,
    framebuffer: vk::Framebuffer,
    imgui_texture_id: TextureId,
}

pub struct VulkanRenderTarget {
    device: Rc<Device>,
    descriptor_manager: Rc<DescriptorManager>,
    allocator: Rc<vk_mem::Allocator>,
    _dub_manager: Arc<DynamicUniformBufferManager>,
    _command_runner: Rc<AdhocCommandRunner>,
    imgui: Rc<RefCell<ImguiRenderer>>,
    command_pool: vk::CommandPool,

    instance: Rc<Instance>,
    physical_device: vk::PhysicalDevice,

    width: u32,
    height: u32,

    render_pass: RenderPass,
    attachments: Option<Attachments>,

    uniform_buffer: Buffer,
    per_frame_descriptor_set: vk::DescriptorSet,

    command_buffer: vk::CommandBuffer,
    render_finished_semaphore: vk::Semaphore,
}

impl VulkanRenderTarget {
    pub fn new(
        width: u32,
        height: u32,
        instance: &Rc<Instance>,
        physical_device: vk::PhysicalDevice,
        device: Rc<Device>,
        allocator: Rc<vk_mem::Allocator>,
        descriptor_manager: Rc<DescriptorManager>,
        dub_manager: Arc<DynamicUniformBufferManager>,
        command_runner: Rc<AdhocCommandRunner>,
        imgui: Rc<RefCell<ImguiRenderer>>,
    ) -> Result<Self, Box<dyn Error>> {
        let command_pool = command_runner.command_pool();
        let depth_fmt = probe_depth_format(&instance.vk_instance(), physical_device, &allocator)?;
        let render_pass = RenderPass::new_offscreen(device.clone(), COLOR_FORMAT, depth_fmt);

        let uniform_buffer = Buffer::new_dynamic_buffer(
            &allocator,
            BufferType::Uniform,
            std::mem::size_of::<PerFrameUniformBuffer>(),
            1,
        )?;

        let per_frame_descriptor_set = descriptor_manager
            .allocate_per_frame_descriptor_sets(std::slice::from_ref(&uniform_buffer))?[0];

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY);
        let command_buffer = device.allocate_command_buffers(&alloc_info)?[0];

        let render_finished_semaphore =
            device.create_semaphore(&vk::SemaphoreCreateInfo::default())?;

        let w = width.max(1);
        let h = height.max(1);
        let attachments = build_attachments(
            &device,
            &descriptor_manager,
            &allocator,
            &instance.vk_instance(),
            physical_device,
            &render_pass,
            &imgui,
            w,
            h,
        )?;

        Ok(Self {
            device,
            descriptor_manager,
            allocator,
            _dub_manager: dub_manager,
            _command_runner: command_runner,
            imgui,
            command_pool,
            instance: Rc::clone(instance),
            physical_device,
            width: w,
            height: h,
            render_pass,
            attachments: Some(attachments),
            uniform_buffer,
            per_frame_descriptor_set,
            command_buffer,
            render_finished_semaphore,
        })
    }

    pub fn vk_render_pass(&self) -> vk::RenderPass {
        self.render_pass.vk_render_pass()
    }

    pub fn vk_framebuffer(&self) -> vk::Framebuffer {
        self.attachments
            .as_ref()
            .map(|a| a.framebuffer)
            .unwrap_or_else(vk::Framebuffer::null)
    }

    pub fn vk_command_buffer(&self) -> vk::CommandBuffer {
        self.command_buffer
    }

    pub fn vk_render_finished_semaphore(&self) -> vk::Semaphore {
        self.render_finished_semaphore
    }

    pub fn per_frame_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_frame_descriptor_set
    }

    pub fn uniform_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.uniform_buffer
    }

    pub fn vk_extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.width,
            height: self.height,
        }
    }
}

impl RenderTarget for VulkanRenderTarget {
    fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if w == self.width && h == self.height {
            return;
        }
        self.device.wait_idle();

        if let Some(old) = self.attachments.take() {
            self.imgui
                .borrow_mut()
                .remove_texture(Some(old.imgui_texture_id));
            self.device.destroy_framebuffer(old.framebuffer);
        }

        let new_attachments = build_attachments(
            &self.device,
            &self.descriptor_manager,
            &self.allocator,
            &self.instance.vk_instance(),
            self.physical_device,
            &self.render_pass,
            &self.imgui,
            w,
            h,
        )
        .expect("VulkanRenderTarget: failed to recreate attachments on resize");

        self.attachments = Some(new_attachments);
        self.width = w;
        self.height = h;
    }

    fn imgui_texture_id(&self) -> u64 {
        self.attachments
            .as_ref()
            .map(|a| a.imgui_texture_id.id() as u64)
            .unwrap_or(0)
    }
}

impl Drop for VulkanRenderTarget {
    fn drop(&mut self) {
        self.device.wait_idle();
        if let Some(old) = self.attachments.take() {
            self.imgui
                .borrow_mut()
                .remove_texture(Some(old.imgui_texture_id));
            self.device.destroy_framebuffer(old.framebuffer);
        }
        if self.render_finished_semaphore != vk::Semaphore::null() {
            self.device
                .destroy_semaphore(self.render_finished_semaphore);
        }
        if self.command_buffer != vk::CommandBuffer::null() {
            self.device
                .free_command_buffers(self.command_pool, &[self.command_buffer]);
        }
        if self.per_frame_descriptor_set != vk::DescriptorSet::null() {
            self.descriptor_manager
                .free_per_frame_descriptor_sets(&[self.per_frame_descriptor_set]);
        }
    }
}

fn probe_depth_format(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    allocator: &Rc<vk_mem::Allocator>,
) -> Result<vk::Format, Box<dyn Error>> {
    // One-shot 1x1 probe to reuse the swapchain's depth-format selection
    // table inside `Image::new_depth_image` without duplicating it.
    let probe = Image::new_depth_image(instance, physical_device, allocator, 1, 1)?;
    Ok(probe.vk_format())
}

#[allow(clippy::too_many_arguments)]
fn build_attachments(
    device: &Rc<Device>,
    descriptor_manager: &Rc<DescriptorManager>,
    allocator: &Rc<vk_mem::Allocator>,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    render_pass: &RenderPass,
    imgui: &Rc<RefCell<ImguiRenderer>>,
    width: u32,
    height: u32,
) -> Result<Attachments, Box<dyn Error>> {
    let color_image = Image::new_color_attachment_image(allocator, width, height)?;
    let color_view = ImageView::new_color_image_view(
        device.clone(),
        color_image.vk_image(),
        color_image.vk_format(),
    )?;
    let depth_image = Image::new_depth_image(instance, physical_device, allocator, width, height)?;
    let depth_view = ImageView::new_depth_image_view(
        device.clone(),
        depth_image.vk_image(),
        depth_image.vk_format(),
    )?;

    let attachments = [color_view.vk_image_view(), depth_view.vk_image_view()];
    let create_info = vk::FramebufferCreateInfo::default()
        .render_pass(render_pass.vk_render_pass())
        .attachments(&attachments)
        .layers(1)
        .width(width)
        .height(height);
    let framebuffer = device.create_framebuffer(&create_info)?;

    let descriptor_set =
        descriptor_manager.create_image_view_descriptor_set(color_view.vk_image_view());
    let texture_id = imgui.borrow_mut().upsert_texture(None, descriptor_set);

    Ok(Attachments {
        _color_image: color_image,
        _color_view: color_view,
        _depth_image: depth_image,
        _depth_view: depth_view,
        framebuffer,
        imgui_texture_id: texture_id,
    })
}
