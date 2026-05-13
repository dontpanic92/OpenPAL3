use super::{
    device::Device, pipeline_layout::PipelineLayout, render_pass::RenderPass, shader::VulkanShader,
};
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::vulkan::material::VulkanMaterial;
use crate::rendering::{BlendMode, CullMode, DepthMode, MaterialKey};
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
            material.key(),
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
        key: &MaterialKey,
    ) -> Result<Vec<vk::Pipeline>, Box<dyn Error>> {
        let entry_point = CString::new("main").unwrap();
        let vert_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader.vk_vert_shader_module());

        // Fragment-shader specialization: constant_id = 0 -> ALPHA_TEST (bool).
        // Set to 1 only for `BlendMode::AlphaTest`; every other mode runs the
        // opaque variant so early-Z is preserved.
        let alpha_test_value: u32 = match key.blend {
            BlendMode::AlphaTest => 1,
            _ => 0,
        };
        let alpha_test_bytes = alpha_test_value.to_ne_bytes();
        let spec_map_entries = [vk::SpecializationMapEntry::default()
            .constant_id(0)
            .offset(0)
            .size(std::mem::size_of::<u32>())];
        let spec_info = vk::SpecializationInfo::default()
            .map_entries(&spec_map_entries)
            .data(&alpha_test_bytes);
        let frag_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader.vk_frag_shader_module())
            .specialization_info(&spec_info);

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
                .cull_mode(match key.cull {
                    CullMode::Back => vk::CullModeFlags::BACK,
                    CullMode::Front => vk::CullModeFlags::FRONT,
                    CullMode::None => vk::CullModeFlags::NONE,
                })
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false);

        let pipeline_multisample_state_create_info =
            vk::PipelineMultisampleStateCreateInfo::default()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // BlendMode -> color-attachment blend state. `AlphaTest` is now
        // blend-disabled: the cutout shader variant runs `discard` so
        // surviving pixels are fully opaque and don't need the blender.
        //
        // The blend factors assume *straight* (non-premultiplied) alpha for
        // source colors — `AlphaBlend` uses `SRC_ALPHA / 1-SRC_ALPHA` for
        // color, and `Additive` weights the source by its own alpha
        // (`SRC_ALPHA / ONE`) so transparent texels contribute nothing.
        // Destination alpha is accumulated with `ONE_MINUS_SRC_ALPHA` so
        // overlapping translucent draws compose to a correct final alpha.
        let (blend_enable, src_color, dst_color, src_alpha, dst_alpha) = match key.blend {
            BlendMode::Opaque | BlendMode::AlphaTest => (
                false,
                vk::BlendFactor::ONE,
                vk::BlendFactor::ZERO,
                vk::BlendFactor::ONE,
                vk::BlendFactor::ZERO,
            ),
            BlendMode::AlphaBlend => (
                true,
                vk::BlendFactor::SRC_ALPHA,
                vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                vk::BlendFactor::ONE,
                vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            ),
            BlendMode::Additive => (
                true,
                vk::BlendFactor::SRC_ALPHA,
                vk::BlendFactor::ONE,
                vk::BlendFactor::ZERO,
                vk::BlendFactor::ONE,
            ),
            BlendMode::Multiply => (
                true,
                vk::BlendFactor::DST_COLOR,
                vk::BlendFactor::ZERO,
                vk::BlendFactor::ZERO,
                vk::BlendFactor::ONE,
            ),
        };

        let pipeline_color_blend_attachment_state =
            vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(
                    vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                )
                .blend_enable(blend_enable)
                .src_color_blend_factor(src_color)
                .dst_color_blend_factor(dst_color)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(src_alpha)
                .dst_alpha_blend_factor(dst_alpha)
                .alpha_blend_op(vk::BlendOp::ADD);

        let attachments = [pipeline_color_blend_attachment_state];
        let pipeline_color_blending_state_create_info =
            vk::PipelineColorBlendStateCreateInfo::default()
                .logic_op_enable(false)
                .attachments(&attachments);

        let (depth_test, depth_write) = match key.depth {
            DepthMode::TestWrite => (true, true),
            DepthMode::TestOnly => (true, false),
            DepthMode::Disabled => (false, false),
        };
        let depth_stencil_state_create_info = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(depth_test)
            .depth_write_enable(depth_write)
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
