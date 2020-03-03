use super::buffer::{Buffer, BufferType};
use super::descriptor_sets::DescriptorSets;
use super::material::VulkanMaterial;
use super::VulkanRenderingEngine;
use crate::rendering::RenderObject;
use ash::vk;
use std::error::Error;

pub struct VulkanRenderObject {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    material: VulkanMaterial,
    per_object_descriptor_sets: DescriptorSets,
}

impl VulkanRenderObject {
    pub fn new(
        object: &RenderObject,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = if object.is_host_dynamic() {
            Buffer::new_dynamic_buffer_with_data(
                engine.allocator(),
                BufferType::Vertex,
                object.vertices().data(),
            )?
        } else {
            engine.create_device_buffer_with_data(BufferType::Vertex, object.vertices().data())?
        };

        let index_buffer =
            engine.create_device_buffer_with_data(BufferType::Index, object.indices())?;

        let material = VulkanMaterial::new(object.material(), engine)?;
        let per_object_descriptor_sets = engine
            .descriptor_manager()
            .allocate_per_object_descriptor_set(&material)?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            material,
            per_object_descriptor_sets,
        })
    }

    pub fn update(&mut self, object: &mut RenderObject) {
        let _ = self
            .vertex_buffer
            .copy_memory_from(object.vertices().data());
        object.is_dirty = false;
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    pub fn material(&self) -> &VulkanMaterial {
        &self.material
    }

    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_object_descriptor_sets.vk_descriptor_set()[0]
    }
}
