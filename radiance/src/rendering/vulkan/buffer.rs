use super::memory::Memory;
use super::VulkanRenderingEngine;
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device, Instance};
use std::error::Error;
use std::mem::size_of;
use std::rc::{Rc, Weak};

pub enum BufferType {
    Index = 0,
    Vertex = 1,
    Uniform = 2,
}

pub struct Buffer {
    device: Weak<Device>,
    buffer: vk::Buffer,
    memory: Memory,
    buffer_size: u64,
    element_count: u32,
}

impl Buffer {
    pub fn new_staging_buffer(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        element_size: usize,
        element_count: usize,
    ) -> Result<Self, Box<dyn Error>> {
        Buffer::new_buffer(
            instance,
            Rc::downgrade(&device),
            physical_device,
            element_size,
            element_count,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
    }

    pub fn new_staging_buffer_with_data<T>(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        data: &[T],
    ) -> Result<Self, Box<dyn Error>> {
        let mut staging_buffer = Buffer::new_buffer(
            instance,
            Rc::downgrade(&device),
            physical_device,
            size_of::<T>(),
            data.len(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        staging_buffer.memory.copy_from(data)?;
        Ok(staging_buffer)
    }

    pub fn new_uniform_buffer(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        element_size: usize,
        element_count: usize,
    ) -> Result<Self, Box<dyn Error>> {
        Buffer::new_buffer(
            instance,
            Rc::downgrade(&device),
            physical_device,
            element_size,
            element_count,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
    }

    pub fn new_device_buffer_with_data<T>(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        data: &Vec<T>,
        buffer_type: BufferType,
        engine: &VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let mut staging_buffer = Buffer::new_buffer(
            instance,
            Rc::downgrade(&device),
            physical_device,
            size_of::<T>(),
            data.len(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        staging_buffer.memory.copy_from(data)?;
        let flags = match buffer_type {
            BufferType::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferType::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferType::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        };

        let mut buffer = Buffer::new_buffer(
            instance,
            Rc::downgrade(&device),
            physical_device,
            size_of::<T>(),
            data.len(),
            flags | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        buffer.copy_from(&staging_buffer, engine)?;
        Ok(buffer)
    }

    pub fn vk_buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn element_count(&self) -> u32 {
        self.element_count
    }

    fn new_buffer(
        instance: &Instance,
        device: Weak<Device>,
        physical_device: vk::PhysicalDevice,
        element_size: usize,
        element_count: usize,
        buffer_usage_flags: vk::BufferUsageFlags,
        memory_prop_flags: vk::MemoryPropertyFlags,
    ) -> Result<Self, Box<dyn Error>> {
        let rc_device = device.upgrade().unwrap();
        let buffer_size = element_count as u64 * element_size as u64; // (size_of::<T>() * data.len()) as u64;
        let buffer = {
            let create_info = vk::BufferCreateInfo::builder()
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .usage(buffer_usage_flags)
                .size(buffer_size)
                .build();
            unsafe { rc_device.create_buffer(&create_info, None)? }
        };

        let memory = Memory::new_for_buffer(
            instance,
            &rc_device,
            physical_device,
            buffer,
            memory_prop_flags,
        )?;
        unsafe { rc_device.bind_buffer_memory(buffer, memory.vk_device_memory(), 0)? };

        Ok(Self {
            device,
            buffer,
            memory,
            buffer_size,
            element_count: element_count as u32,
        })
    }

    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    fn copy_from(&mut self, src_buffer: &Buffer, engine: &VulkanRenderingEngine) -> VkResult<()> {
        engine.run_commands_one_shot(|device, &command_buffer| {
            let copy_region = vk::BufferCopy::builder()
                .size(src_buffer.buffer_size)
                .build();
            unsafe {
                device.cmd_copy_buffer(
                    command_buffer,
                    src_buffer.buffer,
                    self.buffer,
                    &[copy_region],
                );
            }
        })
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            rc_device.destroy_buffer(self.buffer, None);
        }
    }
}
