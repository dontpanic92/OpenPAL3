use super::device::Device;
use crate::rendering::vertex_buffer::{VertexComponents, VertexComponentsLayout};
use crate::rendering::{shader::ShaderProgramData, Shader, ShaderProgram};
use ash::vk;
use std::error::Error;
use std::rc::Rc;

pub struct VulkanShader {
    device: Rc<Device>,
    vertex_component_layout: VertexComponentsLayout,
    vert_shader: vk::ShaderModule,
    frag_shader: vk::ShaderModule,
    name: String,
}

impl Shader for VulkanShader {
    fn name(&self) -> &str {
        &self.name
    }
}

impl VulkanShader {
    pub fn new(shader: ShaderProgram, device: Rc<Device>) -> Result<Self, Box<dyn Error>> {
        let data = get_shader_proram_data(shader);

        let vert_shader = Self::create_shader_module_from_memory(&device, data.vert_src).unwrap();
        let frag_shader = Self::create_shader_module_from_memory(&device, data.frag_src).unwrap();

        Ok(Self {
            device,
            vertex_component_layout: VertexComponentsLayout::from_components(data.components),
            vert_shader,
            frag_shader,
            name: data.name.to_owned(),
        })
    }

    pub fn get_binding_description(&self) -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(self.vertex_component_layout.size() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    // A better way: reflect shader code to get the desciprtions automatically
    pub fn get_attribute_descriptions(&self) -> Vec<vk::VertexInputAttributeDescription> {
        let mut descs = vec![];

        if let Some(position_offset) = self
            .vertex_component_layout
            .get_offset(VertexComponents::POSITION)
        {
            let pos_attr = vk::VertexInputAttributeDescription::builder()
                .offset(position_offset as u32)
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .build();

            descs.push(pos_attr);
        }

        if let Some(normal_offset) = self
            .vertex_component_layout
            .get_offset(VertexComponents::NORMAL)
        {
            let normal_attr = vk::VertexInputAttributeDescription::builder()
                .offset(normal_offset as u32)
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .build();

            descs.push(normal_attr);
        }

        if let Some(texcoord_offset) = self
            .vertex_component_layout
            .get_offset(VertexComponents::TEXCOORD)
        {
            let tex_attr = vk::VertexInputAttributeDescription::builder()
                .offset(texcoord_offset as u32)
                .binding(0)
                .location(2)
                .format(vk::Format::R32G32_SFLOAT)
                .build();

            descs.push(tex_attr);
        }

        if let Some(texcoord2_offset) = self
            .vertex_component_layout
            .get_offset(VertexComponents::TEXCOORD2)
        {
            let texcoord2_attr = vk::VertexInputAttributeDescription::builder()
                .offset(texcoord2_offset as u32)
                .binding(0)
                .location(3)
                .format(vk::Format::R32G32_SFLOAT)
                .build();

            descs.push(texcoord2_attr);
        }

        descs
    }

    pub fn vk_vert_shader_module(&self) -> vk::ShaderModule {
        self.vert_shader
    }

    pub fn vk_frag_shader_module(&self) -> vk::ShaderModule {
        self.frag_shader
    }

    fn create_shader_module_from_memory(
        device: &Rc<Device>,
        code: &[u8],
    ) -> Result<vk::ShaderModule, Box<dyn Error>> {
        let code_u32 =
            unsafe { std::slice::from_raw_parts::<u32>(code.as_ptr().cast(), code.len() / 4) };
        let create_info = vk::ShaderModuleCreateInfo::builder().code(code_u32).build();
        Ok(device.create_shader_module(&create_info)?)
    }
}

impl Drop for VulkanShader {
    fn drop(&mut self) {
        self.device.destroy_shader_module(self.vert_shader);
        self.device.destroy_shader_module(self.frag_shader);
    }
}

static SIMPLE_TRIANGLE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.vert.spv"));
static SIMPLE_TRIANGLE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.frag.spv"));
static LIGHTMAP_TEXTURE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.vert.spv"));
static LIGHTMAP_TEXTURE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.frag.spv"));

fn get_shader_proram_data(shader: ShaderProgram) -> ShaderProgramData {
    match shader {
        ShaderProgram::TexturedNoLight => ShaderProgramData::new(
            "simple_shader",
            SIMPLE_TRIANGLE_VERT,
            SIMPLE_TRIANGLE_FRAG,
            VertexComponents::POSITION | VertexComponents::TEXCOORD,
        ),
        ShaderProgram::TexturedLightmap => ShaderProgramData::new(
            "lightmap_texture",
            LIGHTMAP_TEXTURE_VERT,
            LIGHTMAP_TEXTURE_FRAG,
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2,
        ),
    }
}
