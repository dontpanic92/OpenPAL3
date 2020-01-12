use super::buffer::{Buffer, BufferType};
use super::VulkanRenderingEngine;
use crate::rendering::RenderObject;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::error::Error;
use std::rc::Weak;

pub struct VulkanRenderObject {
    device: Weak<Device>,
    command_pool: Weak<vk::CommandPool>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    command_buffers: Vec<vk::CommandBuffer>,
}

impl VulkanRenderObject {
    pub fn new(
        engine: &VulkanRenderingEngine,
        object: &RenderObject,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = engine.create_buffer(BufferType::Vertex, &object.vertices())?;
        let index_buffer = engine.create_buffer(BufferType::Index, &object.indices())?;
        let command_buffers = engine.create_command_buffers(&vertex_buffer, &index_buffer)?;

        Ok(Self {
            device: engine.device(),
            command_pool: engine.command_pool(),
            vertex_buffer,
            index_buffer,
            command_buffers,
        })
    }

    pub fn command_buffers(&self) -> &Vec<vk::CommandBuffer> {
        &self.command_buffers
    }

    pub fn recreate_command_buffers(
        &mut self,
        engine: &VulkanRenderingEngine,
    ) -> Result<(), vk::Result> {
        self.free_command_buffers();
        self.command_buffers =
            engine.create_command_buffers(&self.vertex_buffer, &self.index_buffer)?;

        Ok(())
    }

    fn free_command_buffers(&mut self) {
        let rc_device = self.device.upgrade().unwrap();
        let rc_command_pool = self.command_pool.upgrade().unwrap();
        unsafe {
            rc_device.free_command_buffers(*rc_command_pool, &self.command_buffers);
        }
    }
}

impl Drop for VulkanRenderObject {
    fn drop(&mut self) {
        self.free_command_buffers();
    }
}
