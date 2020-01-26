use super::error::VulkanBackendError;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk::{DeviceMemory, PhysicalDevice};
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
    memory: DeviceMemory,
    buffer_size: u64,
    element_count: u32,
}

impl Buffer {
    pub fn new_uniform_buffer(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: PhysicalDevice,
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
        physical_device: PhysicalDevice,
        data: &Vec<T>,
        buffer_type: BufferType,
        command_pool: &vk::CommandPool,
        queue: vk::Queue,
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

        staging_buffer.copy_memory_from(data)?;
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

        buffer.copy_buffer_from(&staging_buffer, command_pool, queue)?;
        Ok(buffer)
    }

    pub fn copy_memory_from<T>(&mut self, data: &[T]) -> ash::prelude::VkResult<()> {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            let dst = rc_device.map_memory(
                self.memory,
                0,
                self.buffer_size,
                vk::MemoryMapFlags::default(),
            )?;
            std::ptr::copy(
                data.as_ptr() as *const std::ffi::c_void,
                dst,
                self.buffer_size as usize,
            );
            rc_device.unmap_memory(self.memory);
        }

        Ok(())
    }

    pub fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn element_count(&self) -> u32 {
        self.element_count
    }

    fn new_buffer(
        instance: &Instance,
        device: Weak<Device>,
        physical_device: PhysicalDevice,
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

        let memory = {
            let requirements = unsafe { rc_device.get_buffer_memory_requirements(buffer) };
            let properties =
                unsafe { instance.get_physical_device_memory_properties(physical_device) };
            let index = Buffer::get_memory_type_index(
                properties,
                requirements.memory_type_bits,
                memory_prop_flags,
            )?;

            let create_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(requirements.size)
                .memory_type_index(index as u32)
                .build();

            unsafe { rc_device.allocate_memory(&create_info, None)? }
        };

        unsafe { rc_device.bind_buffer_memory(buffer, memory, 0)? };

        Ok(Self {
            device,
            buffer,
            memory,
            buffer_size,
            element_count: element_count as u32,
        })
    }

    fn copy_buffer_from(
        &mut self,
        src_buffer: &Buffer,
        command_pool: &vk::CommandPool,
        queue: vk::Queue,
    ) -> ash::prelude::VkResult<()> {
        let rc_device = self.device.upgrade().unwrap();
        let command_buffer = {
            let allocation_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(*command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1)
                .build();
            unsafe { rc_device.allocate_command_buffers(&allocation_info)? }
        };

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe {
            rc_device.begin_command_buffer(command_buffer[0], &begin_info)?;
            let copy_region = vk::BufferCopy::builder()
                .size(src_buffer.buffer_size)
                .build();
            rc_device.cmd_copy_buffer(
                command_buffer[0],
                src_buffer.buffer,
                self.buffer,
                &[copy_region],
            );
            rc_device.end_command_buffer(command_buffer[0])?;

            let submit_info = vk::SubmitInfo::builder()
                .command_buffers(&command_buffer)
                .build();
            rc_device.queue_submit(queue, &[submit_info], vk::Fence::default())?;
            rc_device.queue_wait_idle(queue)?;

            rc_device.free_command_buffers(*command_pool, &command_buffer);
        }

        Ok(())
    }

    fn get_memory_type_index(
        properties: vk::PhysicalDeviceMemoryProperties,
        candidates: u32,
        required_properties: vk::MemoryPropertyFlags,
    ) -> Result<usize, VulkanBackendError> {
        let index = properties
            .memory_types
            .iter()
            .take(properties.memory_type_count as usize)
            .enumerate()
            .filter_map(|(i, t)| {
                if (candidates & (1 << i as u32) != 0)
                    && (t.property_flags & required_properties == required_properties)
                {
                    Some(i)
                } else {
                    None
                }
            })
            .take(1)
            .collect::<Vec<usize>>();

        if index.is_empty() {
            Err(VulkanBackendError::NoSuitableMemoryFound)
        } else {
            Ok(index[0])
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            rc_device.destroy_buffer(self.buffer, None);
            rc_device.free_memory(self.memory, None);
        }
    }
}
