use super::{
    device::Device, pipeline_layout::PipelineLayout, render_pass::RenderPass, shader::VulkanShader,
};
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::vulkan::material::VulkanMaterial;
use ash::vk;
use std::error::Error;
use std::ffi::CString;
use std::rc::Rc;

pub struct Pipeline {
    device: Rc<Device>,
    pipeline: vk::Pipeline,
    pipeline_layout: PipelineLayout,
}

impl Pipeline {
    pub fn new(
        device: Rc<Device>,
        descriptor_manager: &DescriptorManager,
        render_pass: &RenderPass,
        material: &VulkanMaterial,
        extent: vk::Extent2D,
    ) -> Self {
        let descriptor_set_layouts = descriptor_manager.get_vk_descriptor_set_layouts(material);
        let pipeline_layout = PipelineLayout::new(device.clone(), &descriptor_set_layouts);
        let pipeline = Self::create_pipeline(
            &device,
            render_pass.vk_render_pass(),
            pipeline_layout.vk_pipeline_layout(),
            &extent,
            material.shader(),
        )
        .unwrap()[0];

        Self {
            device,
            pipeline,
            pipeline_layout,
        }
    }

    pub fn pipeline_layout(&self) -> &PipelineLayout {
        &self.pipeline_layout
    }

    pub fn vk_pipeline(&self) -> vk::Pipeline {
        self.pipeline
    }

    fn create_pipeline(
        device: &Rc<Device>,
        render_pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        extent: &vk::Extent2D,
        shader: &VulkanShader,
    ) -> Result<Vec<vk::Pipeline>, Box<dyn Error>> {
        let entry_point = CString::new("main").unwrap();
        let vert_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader.vk_vert_shader_module());
        let frag_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader.vk_frag_shader_module());

        let binding_descriptions = [shader.get_binding_description()];
        let attribute_descriptions = shader.get_attribute_descriptions();
        let pipeline_vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&attribute_descriptions)
            .vertex_binding_descriptions(&binding_descriptions);

        let pipeline_input_assembly_create_info =
            vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                .primitive_restart_enable(false);

        let viewport = vk::Viewport::default()
            .x(0f32)
            .y(0f32)
            .min_depth(0f32)
            .max_depth(1f32)
            .height(extent.height as f32)
            .width(extent.width as f32);

        let scissor = vk::Rect2D::default()
            .extent(*extent)
            .offset(vk::Offset2D::default().x(0).y(0));

        let viewports = [viewport];
        let scissors = [scissor];
        let pipeline_viewport_state_create_info = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        let pipeline_rasterization_state_create_info =
            vk::PipelineRasterizationStateCreateInfo::default()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1f32)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false);

        let pipeline_multisample_state_create_info =
            vk::PipelineMultisampleStateCreateInfo::default()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let pipeline_color_blend_attachment_state =
            vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(
                    vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                )
                .blend_enable(true)
                .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD);

        let attachments = [pipeline_color_blend_attachment_state];
        let pipeline_color_blending_state_create_info =
            vk::PipelineColorBlendStateCreateInfo::default()
                .logic_op_enable(false)
                .attachments(&attachments);

        let depth_stencil_state_create_info = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let dynamic_state_create_info = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT]);

        let stages = [vert_shader_stage_create_info, frag_shader_stage_create_info];
        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&pipeline_vertex_input_create_info)
            .input_assembly_state(&pipeline_input_assembly_create_info)
            .dynamic_state(&dynamic_state_create_info)
            .viewport_state(&pipeline_viewport_state_create_info)
            .rasterization_state(&pipeline_rasterization_state_create_info)
            .multisample_state(&pipeline_multisample_state_create_info)
            .color_blend_state(&pipeline_color_blending_state_create_info)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0)
            .depth_stencil_state(&depth_stencil_state_create_info);

        let pipelines = match device.create_graphics_pipelines(&[create_info]) {
            Ok(p) => Ok(p),
            Err((p, e)) => {
                for x in p.into_iter() {
                    device.destroy_pipeline(x);
                }

                Err(Box::new(e) as Box<dyn Error>)
            }
        };

        return pipelines;
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.device.destroy_pipeline(self.pipeline);
    }
}
