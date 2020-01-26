use super::buffer::{Buffer, BufferType};
use super::VulkanRenderingEngine;
use crate::rendering::RenderObject;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::error::Error;
use std::rc::Weak;

pub struct VulkanRenderObject {
    device: Weak<Device>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl VulkanRenderObject {
    pub fn new(
        engine: &VulkanRenderingEngine,
        object: &RenderObject,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = engine.create_buffer(BufferType::Vertex, &object.vertices())?;
        let index_buffer = engine.create_buffer(BufferType::Index, &object.indices())?;

        Ok(Self {
            device: engine.device(),
            vertex_buffer,
            index_buffer,
        })
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }
}
