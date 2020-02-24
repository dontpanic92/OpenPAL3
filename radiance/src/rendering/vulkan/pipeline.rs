
use std::error::Error;
use ash::{vk, Device};
use std::ffi::CString;
use ash::version::DeviceV1_0;

pub struct Pipeline {

}

impl Pipeline {
    pub fn new() -> Self {

    }

    fn create_vk_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        extent: &vk::Extent2D,
    ) -> Result<Vec<vk::Pipeline>, Box<dyn Error>> {
        let vert_shader = create_shader_module_from_array(device, SIMPLE_TRIANGLE_VERT)?;
        let frag_shader = create_shader_module_from_array(device, SIMPLE_TRIANGLE_FRAG)?;
    
        let entry_point = CString::new("main").unwrap();
        let vert_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_shader)
            .build();
        let frag_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .name(&entry_point)
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_shader)
            .build();
    
        let binding_descriptions = [super::vertex_helper::get_binding_description()];
        let attribute_descriptions = super::vertex_helper::get_attribute_descriptions();
        let pipeline_vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&attribute_descriptions)
            .vertex_binding_descriptions(&binding_descriptions)
            .build();
    
        let pipeline_input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false)
            .build();
    
        let viewport = vk::Viewport::builder()
            .x(0f32)
            .y(0f32)
            .min_depth(0f32)
            .max_depth(1f32)
            .height(extent.height as f32)
            .width(extent.width as f32)
            .build();
    
        let scissor = vk::Rect2D::builder()
            .extent(*extent)
            .offset(vk::Offset2D::builder().x(0).y(0).build())
            .build();
    
        let viewports = [viewport];
        let scissors = [scissor];
        let pipeline_viewport_state_create_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors)
            .build();
    
        let pipeline_rasterization_state_create_info =
            vk::PipelineRasterizationStateCreateInfo::builder()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1f32)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false)
                .build();
    
        let pipeline_multisample_state_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .build();
    
        let pipeline_color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            .blend_enable(false)
            .build();
    
        let attachments = [pipeline_color_blend_attachment_state];
        let pipeline_color_blending_state_create_info =
            vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op_enable(false)
                .attachments(&attachments)
                .build();
    
        let depth_stencil_state_create_info = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .build();
    
        let stages = [vert_shader_stage_create_info, frag_shader_stage_create_info];
        let create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&stages)
            .vertex_input_state(&pipeline_vertex_input_create_info)
            .input_assembly_state(&pipeline_input_assembly_create_info)
            .viewport_state(&pipeline_viewport_state_create_info)
            .rasterization_state(&pipeline_rasterization_state_create_info)
            .multisample_state(&pipeline_multisample_state_create_info)
            .color_blend_state(&pipeline_color_blending_state_create_info)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0)
            .depth_stencil_state(&depth_stencil_state_create_info)
            .build();
    
        let pipelines = match unsafe {
            device.create_graphics_pipelines(vk::PipelineCache::default(), &[create_info], None)
        } {
            Ok(p) => Ok(p),
            Err((p, e)) => {
                for x in p.into_iter() {
                    unsafe { device.destroy_pipeline(x, None) };
                }
    
                Err(Box::new(e) as Box<dyn Error>)
            }
        };
    
        unsafe {
            device.destroy_shader_module(vert_shader, None);
            device.destroy_shader_module(frag_shader, None);
        }
    
        return pipelines;
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {

    }
}
