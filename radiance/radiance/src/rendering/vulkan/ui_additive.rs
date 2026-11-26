use std::{io::Cursor, rc::Rc};

use ash::vk;

use crate::imgui::additive::AdditiveImage;
use crate::rendering::vulkan::{
    buffer::{Buffer, BufferType},
    device::Device,
    vulkan_engine,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

pub(super) struct AdditiveUi {
    device: Rc<Device>,
    allocator: Rc<vk_mem::Allocator>,
    layout: vk::PipelineLayout,
    descriptor_layout: vk::DescriptorSetLayout,
    pipeline: vk::Pipeline,
    buffers: Vec<Option<Buffer>>,
    frame: usize,
}

impl AdditiveUi {
    pub fn new(
        device: Rc<Device>,
        allocator: Rc<vk_mem::Allocator>,
        render_pass: vk::RenderPass,
    ) -> Self {
        let binding = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let descriptor_layout = device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding),
            )
            .unwrap();
        let layouts = [descriptor_layout];
        let layout = device
            .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts))
            .unwrap();
        let mut result = Self {
            device,
            allocator,
            layout,
            descriptor_layout,
            pipeline: vk::Pipeline::null(),
            buffers: (0..vulkan_engine::MAX_FRAMES_IN_FLIGHT)
                .map(|_| None)
                .collect(),
            frame: 0,
        };
        result.set_render_pass(render_pass);
        result
    }

    pub fn set_render_pass(&mut self, render_pass: vk::RenderPass) {
        let modules = [
            include_bytes!(concat!(env!("OUT_DIR"), "/ui_additive.vert.spv")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/ui_additive.frag.spv")).as_slice(),
        ]
        .map(|bytes| {
            let code = ash::util::read_spv(&mut Cursor::new(bytes)).unwrap();
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code))
                .unwrap()
        });
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(modules[0])
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(modules[1])
                .name(c"main"),
        ];
        let bindings = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)];
        let attributes = [
            vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(8),
            vk::VertexInputAttributeDescription::default()
                .location(2)
                .binding(0)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(16),
        ];
        let input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attributes);
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .line_width(1.);
        let samples = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let depth = vk::PipelineDepthStencilStateCreateInfo::default();
        let attachments = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&input)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&samples)
            .depth_stencil_state(&depth)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic)
            .layout(self.layout)
            .render_pass(render_pass);
        let pipeline = self.device.create_graphics_pipelines(&[info]).unwrap()[0];
        self.device.destroy_pipeline(self.pipeline);
        self.pipeline = pipeline;
        for module in modules {
            self.device.destroy_shader_module(module);
        }
    }

    pub fn draw(
        &mut self,
        command: vk::CommandBuffer,
        data: &imgui::DrawData,
        images: &[AdditiveImage],
        descriptors: &[Option<vk::DescriptorSet>],
    ) {
        self.frame = (self.frame + 1) % self.buffers.len();
        if images.is_empty() || data.display_size[0] <= 0. || data.display_size[1] <= 0. {
            return;
        }
        let mut vertices = Vec::with_capacity(images.len() * 6);
        for image in images {
            let [x0, y0, x1, y1] = image.rect;
            let [u0, v0, u1, v1] = image.uv;
            for (x, y, u, v) in [
                (x0, y0, u0, v0),
                (x1, y0, u1, v0),
                (x1, y1, u1, v1),
                (x0, y0, u0, v0),
                (x1, y1, u1, v1),
                (x0, y1, u0, v1),
            ] {
                vertices.push(Vertex {
                    position: [
                        (x - data.display_pos[0]) * 2. / data.display_size[0] - 1.,
                        (y - data.display_pos[1]) * 2. / data.display_size[1] - 1.,
                    ],
                    uv: [u, v],
                    color: image.color,
                });
            }
        }
        let buffer = &mut self.buffers[self.frame];
        if buffer
            .as_ref()
            .is_none_or(|buffer| buffer.element_count() < vertices.len() as u32)
        {
            *buffer = Some(
                Buffer::new_dynamic_buffer_with_data(
                    &self.allocator,
                    BufferType::Vertex,
                    &vertices,
                )
                .unwrap(),
            );
        } else {
            buffer.as_mut().unwrap().copy_memory_from(&vertices);
        }
        let size = [
            data.display_size[0] * data.framebuffer_scale[0],
            data.display_size[1] * data.framebuffer_scale[1],
        ];
        let vk = self.device.vk_device();
        unsafe {
            vk.cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            vk.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport::default()
                    .width(size[0])
                    .height(size[1])
                    .max_depth(1.)],
            );
            vk.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D::default().extent(vk::Extent2D {
                    width: size[0] as u32,
                    height: size[1] as u32,
                })],
            );
            vk.cmd_bind_vertex_buffers(command, 0, &[buffer.as_ref().unwrap().vk_buffer()], &[0]);
            for (i, descriptor) in descriptors.iter().enumerate() {
                if let Some(descriptor) = descriptor {
                    vk.cmd_bind_descriptor_sets(
                        command,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layout,
                        0,
                        &[*descriptor],
                        &[],
                    );
                    vk.cmd_draw(command, 6, 1, i as u32 * 6, 0);
                }
            }
        }
    }
}

impl Drop for AdditiveUi {
    fn drop(&mut self) {
        self.device.destroy_pipeline(self.pipeline);
        self.device.destroy_pipeline_layout(self.layout);
        self.device
            .destroy_descriptor_set_layout(self.descriptor_layout);
    }
}
