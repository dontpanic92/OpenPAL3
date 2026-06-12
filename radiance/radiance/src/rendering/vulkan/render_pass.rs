use ash::{prelude::VkResult, vk};
use std::rc::Rc;

use super::device::Device;

pub struct RenderPass {
    device: Rc<Device>,
    render_pass: vk::RenderPass,
}

impl RenderPass {
    pub fn new(device: Rc<Device>, color_format: vk::Format, depth_format: vk::Format) -> Self {
        Self::new_with_color_final_layout(
            device,
            color_format,
            depth_format,
            vk::ImageLayout::PRESENT_SRC_KHR,
        )
    }

    /// Render pass whose color attachment ends in
    /// `SHADER_READ_ONLY_OPTIMAL` so subsequent passes (imgui) can sample
    /// it. Used by offscreen render targets.
    pub fn new_offscreen(
        device: Rc<Device>,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> Self {
        Self::new_with_color_final_layout(
            device,
            color_format,
            depth_format,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )
    }

    /// Render pass whose color attachment ends in
    /// `TRANSFER_SRC_OPTIMAL` so the immediate next command can
    /// `vkCmdBlitImage` it into the swapchain image without an
    /// intermediate layout-transition barrier. Used by the
    /// Logical-extent scene pass in `SwapChain` when
    /// `SceneScaleMode::Logical` is active.
    pub fn new_offscreen_blit_src(
        device: Rc<Device>,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> Self {
        Self::new_with_color_final_layout(
            device,
            color_format,
            depth_format,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        )
    }

    /// Render pass for an imgui-only pass that draws on top of an
    /// already-populated swapchain image. Color attachment uses
    /// `LOAD` op (preserve the underlying blit result),
    /// `initial_layout = COLOR_ATTACHMENT_OPTIMAL` (caller barriers
    /// from `TRANSFER_DST_OPTIMAL`), `final_layout = PRESENT_SRC_KHR`.
    /// No depth attachment — imgui doesn't sample depth and skipping
    /// the depth attach saves memory + recreate cost.
    pub fn new_imgui_only(device: Rc<Device>, color_format: vk::Format) -> Self {
        let render_pass = Self::create_imgui_only_render_pass(&device, color_format).unwrap();
        Self {
            device,
            render_pass,
        }
    }

    pub fn vk_render_pass(&self) -> vk::RenderPass {
        self.render_pass
    }

    fn new_with_color_final_layout(
        device: Rc<Device>,
        color_format: vk::Format,
        depth_format: vk::Format,
        color_final_layout: vk::ImageLayout,
    ) -> Self {
        let render_pass =
            Self::create_render_pass(&device, color_format, depth_format, color_final_layout)
                .unwrap();
        Self {
            device,
            render_pass,
        }
    }

    fn create_imgui_only_render_pass(
        device: &Rc<Device>,
        color_format: vk::Format,
    ) -> VkResult<vk::RenderPass> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_reference = vk::AttachmentReference::default()
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .attachment(0);

        let color_attachments = [color_attachment_reference];
        let subpass_description = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments);

        // External → subpass: wait for the prior blit (TRANSFER stage,
        // TRANSFER_WRITE access) to finish before any color writes from
        // imgui draws begin.
        let subpass_dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::TRANSFER)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );

        let attachments = [color_attachment];
        let subpasses = [subpass_description];
        let dependencies = [subpass_dependency];
        let render_pass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        device.create_render_pass(&render_pass_create_info)
    }

    fn create_render_pass(
        device: &Rc<Device>,
        color_format: vk::Format,
        depth_format: vk::Format,
        color_final_layout: vk::ImageLayout,
    ) -> VkResult<vk::RenderPass> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(color_final_layout);

        let depth_attachment = vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let color_attachment_reference = vk::AttachmentReference::default()
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .attachment(0);
        let depth_attachment_reference = vk::AttachmentReference::default()
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .attachment(1);

        let color_attachments = [color_attachment_reference];
        let subpass_description = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments)
            .depth_stencil_attachment(&depth_attachment_reference);

        let subpass_dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::default())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );

        let attachments = [color_attachment, depth_attachment];
        let subpasses = [subpass_description];
        let dependencies = [subpass_dependency];
        let render_pass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        device.create_render_pass(&render_pass_create_info)
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        self.device.destroy_render_pass(self.render_pass);
    }
}
