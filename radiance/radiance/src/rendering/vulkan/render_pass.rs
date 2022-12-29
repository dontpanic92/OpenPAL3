use ash::{prelude::VkResult, vk};
use std::rc::Rc;

use super::device::Device;

pub struct RenderPass {
    device: Rc<Device>,
    render_pass: vk::RenderPass,
}

impl RenderPass {
    pub fn new(device: Rc<Device>, color_format: vk::Format, depth_format: vk::Format) -> Self {
        let render_pass = Self::create_render_pass(&device, color_format, depth_format).unwrap();

        Self {
            device,
            render_pass,
        }
    }

    pub fn vk_render_pass(&self) -> vk::RenderPass {
        self.render_pass
    }

    fn create_render_pass(
        device: &Rc<Device>,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> VkResult<vk::RenderPass> {
        let color_attachment = vk::AttachmentDescription::builder()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let depth_attachment = vk::AttachmentDescription::builder()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let color_attachment_reference = vk::AttachmentReference::builder()
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .attachment(0)
            .build();
        let depth_attachment_reference = vk::AttachmentReference::builder()
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .attachment(1)
            .build();

        let color_attachments = [color_attachment_reference];
        let subpass_description = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments)
            .depth_stencil_attachment(&depth_attachment_reference)
            .build();

        let subpass_dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::default())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .build();

        let attachments = [color_attachment, depth_attachment];
        let subpasses = [subpass_description];
        let dependencies = [subpass_dependency];
        let render_pass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies)
            .build();

        device.create_render_pass(&render_pass_create_info)
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        self.device.destroy_render_pass(self.render_pass);
    }
}
