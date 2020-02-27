use super::adhoc_command_runner::AdhocCommandRunner;
use super::buffer::{Buffer, BufferType};
use super::descriptor_sets::DescriptorSets;
use super::material::VulkanMaterial;
use super::VulkanRenderingEngine;
use crate::rendering::RenderObject;
use ash::vk;
use std::error::Error;
use std::rc::{Rc, Weak};

pub struct VulkanRenderObject {
    vertex_staging_buffer: Buffer,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    material: VulkanMaterial,
    per_object_descriptor_sets: DescriptorSets,
    command_runner: Weak<AdhocCommandRunner>,
}

impl VulkanRenderObject {
    pub fn new(
        object: &RenderObject,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_staging_buffer =
            engine.create_staging_buffer_with_data(object.vertices().data())?;
        let vertex_buffer =
            engine.create_device_buffer_with_data(BufferType::Vertex, object.vertices().data())?;
        let index_buffer =
            engine.create_device_buffer_with_data(BufferType::Index, object.indices())?;

        let command_runner = Rc::downgrade(engine.adhoc_command_runner());
        let material = VulkanMaterial::new(object.material(), engine)?;
        let per_object_descriptor_sets = engine
            .descriptor_manager()
            .allocate_per_object_descriptor_set(material.image_view(), material.sampler())?;

        Ok(Self {
            vertex_staging_buffer,
            vertex_buffer,
            index_buffer,
            material,
            per_object_descriptor_sets,
            command_runner,
        })
    }

    pub fn update(&mut self, object: &mut RenderObject) {
        if let Some(command_runner) = self.command_runner.upgrade() {
            let _ = self
                .vertex_staging_buffer
                .memory_mut()
                .copy_from(object.vertices().data());
            let _ = self
                .vertex_buffer
                .copy_from(&self.vertex_staging_buffer, command_runner.as_ref());

            object.is_dirty = false;
        }
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
