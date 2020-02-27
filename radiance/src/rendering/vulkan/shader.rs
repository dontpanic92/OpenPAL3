use crate::rendering::vertex_buffer::{VertexComponents, VertexMetadata};
use crate::rendering::Shader;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use std::error::Error;
use std::rc::{Rc, Weak};
use std::sync::Arc;

pub struct VulkanShader {
    device: Weak<Device>,
    vertex_metadata: Arc<VertexMetadata>,
    vert_shader: vk::ShaderModule,
    frag_shader: vk::ShaderModule,
    name: String,
}

impl VulkanShader {
    pub fn new(device: &Rc<Device>, shader: &dyn Shader) -> Result<Self, Box<dyn Error>> {
        let vert_shader =
            Self::create_shader_module_from_memory(&device, shader.vert_src()).unwrap();
        let frag_shader =
            Self::create_shader_module_from_memory(&device, shader.frag_src()).unwrap();

        Ok(Self {
            device: Rc::downgrade(&device),
            vertex_metadata: VertexMetadata::get(shader.vertex_components()),
            vert_shader,
            frag_shader,
            name: shader.name().to_owned(),
        })
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn get_binding_description(&self) -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(self.vertex_metadata.size as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    // A better way: reflect the shader code to get the desciprtions automatically
    pub fn get_attribute_descriptions(&self) -> Vec<vk::VertexInputAttributeDescription> {
        let mut descs = vec![];

        if let Some(&position_offset) = self
            .vertex_metadata
            .offsets
            .get(&VertexComponents::POSITION)
        {
            let pos_attr = vk::VertexInputAttributeDescription::builder()
                .offset(position_offset as u32)
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .build();

            descs.push(pos_attr);
        }

        if let Some(&texcoord_offset) = self
            .vertex_metadata
            .offsets
            .get(&VertexComponents::TEXCOORD)
        {
            let tex_attr = vk::VertexInputAttributeDescription::builder()
                .offset(texcoord_offset as u32)
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32_SFLOAT)
                .build();

            descs.push(tex_attr);
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
        unsafe { Ok(device.create_shader_module(&create_info, None)?) }
    }
}

impl Drop for VulkanShader {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_shader_module(self.vert_shader, None);
            device.destroy_shader_module(self.frag_shader, None);
        }
    }
}
