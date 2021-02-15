use super::buffer::{Buffer, BufferType};
use super::material::VulkanMaterial;
use super::uniform_buffers::DynamicUniformBufferManager;
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::{Material, RenderObject, VertexBuffer};
use ash::vk;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;

pub struct VulkanRenderObject {
    vertices: VertexBuffer,
    indices: Vec<u32>,
    host_dynamic: bool,
    dirty: bool,

    dub_manager: Arc<DynamicUniformBufferManager>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    material: Box<VulkanMaterial>,
    per_object_descriptor_sets: vk::DescriptorSet,
    dub_index: usize,
}

impl RenderObject for VulkanRenderObject {
    fn update_vertices(&mut self, updater: &mut dyn FnMut(&mut VertexBuffer)) {
        updater(&mut self.vertices);
        let _ = self.vertex_buffer.copy_memory_from(self.vertices.data());
    }
}

impl VulkanRenderObject {
    pub fn new(
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Box<dyn Material>,
        host_dynamic: bool,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
        dub_manager: &Arc<DynamicUniformBufferManager>,
        descriptor_manager: &DescriptorManager,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = if host_dynamic {
            Buffer::new_dynamic_buffer_with_data(allocator, BufferType::Vertex, vertices.data())?
        } else {
            Buffer::new_device_buffer_with_data(
                allocator,
                BufferType::Vertex,
                vertices.data(),
                command_runner,
            )?
        };

        let index_buffer = Buffer::new_device_buffer_with_data(
            allocator,
            BufferType::Index,
            &indices,
            command_runner,
        )?;

        let material = material.downcast::<VulkanMaterial>().unwrap();
        let per_object_descriptor_sets =
            descriptor_manager.allocate_per_object_descriptor_set(&material)?;
        let dub_index = dub_manager.allocate_buffer();

        Ok(Self {
            vertices: vertices.clone(),
            indices: indices.clone(),
            material,
            host_dynamic,
            dirty: false,
            dub_manager: dub_manager.clone(),
            vertex_buffer,
            index_buffer,
            per_object_descriptor_sets,
            dub_index,
        })
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    pub fn dub_index(&self) -> usize {
        self.dub_index
    }

    pub fn material(&self) -> &VulkanMaterial {
        &self.material
    }

    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_object_descriptor_sets
    }
}

impl Drop for VulkanRenderObject {
    fn drop(&mut self) {
        self.dub_manager.deallocate_buffer(self.dub_index);
    }
}
