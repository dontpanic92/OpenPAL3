use std::rc::{Rc, Weak};
use std::error::Error;
use ash::Device;
use ash::version::DeviceV1_0;
use crate::rendering::{Vertex, Shader};
use crate::rendering::vertex::VertexMetadata;
use ash::vk;
use std::mem::size_of;

pub struct VulkanShader {
    device: Weak<Device>,
    vertex_metadata: &'static VertexMetadata,
    vert_shader: vk::ShaderModule,
    frag_shader: vk::ShaderModule,
}

impl VulkanShader {
    pub fn new(
        device: Rc<Device>,
        shader: &dyn Shader,
    ) -> Result<Self, Box<dyn Error>> {
        let vert_shader = VulkanShader::create_shader_module_from_memory(&device, shader.vert_src()).unwrap();
        let frag_shader = VulkanShader::create_shader_module_from_memory(&device, shader.frag_src()).unwrap();

        Ok(Self {
            device: Rc::downgrade(&device),
            vertex_metadata: VertexMetadata::get(shader.vertex_components()),
            vert_shader,
            frag_shader,
        })
    }

    pub fn get_binding_description(&self) -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(self.vertex_metadata.size as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }
    
    pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        let pos_attr = vk::VertexInputAttributeDescription::builder()
            .offset(Vertex::position_offset() as u32)
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .build();
    
        let tex_attr = vk::VertexInputAttributeDescription::builder()
            .offset(Vertex::tex_coord_offset() as u32)
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .build();
    
        [pos_attr, tex_attr]
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
