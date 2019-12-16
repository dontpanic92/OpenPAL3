use crate::rendering::Vertex;
use super::error::VulkanBackendError;
use std::mem::size_of;
use std::error::Error;
use std::rc::Weak;
use ash::{ vk, Instance, Device };
use ash::vk::{ PhysicalDevice, Buffer, DeviceMemory };
use ash::version::{ InstanceV1_0, DeviceV1_0 };

pub struct VertexBuffer {
    device: Weak<Device>,
    buffer: Buffer,
    memory: DeviceMemory,
}

impl VertexBuffer {
    pub fn new(
        instance: &Instance,
        device: Weak<Device>,
        physical_device: PhysicalDevice,
        vertices: &Vec<Vertex>)
    -> Result<Self, Box<dyn Error>> {
        let rc_device = device.upgrade().unwrap();
        let buffer_size = (size_of::<Vertex>() * vertices.len()) as u64;
        let buffer = {
            let create_info = vk::BufferCreateInfo::builder()
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                .size(buffer_size)
                .build();
            unsafe { rc_device.create_buffer(&create_info, None)? }
        };

        let memory = {
            let requirements = unsafe { rc_device.get_buffer_memory_requirements(buffer) };
            let properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
            let index = VertexBuffer::get_memory_type_index(
                properties,
                requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;

            let create_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(requirements.size)
                .memory_type_index(index as u32)
                .build();
            
            unsafe { rc_device.allocate_memory(&create_info, None)? }
        };

        unsafe { 
            rc_device.bind_buffer_memory(buffer, memory, 0);
            let dst =  rc_device.map_memory(memory, 0, buffer_size, vk::MemoryMapFlags::default())?;
            std::ptr::copy(vertices.as_ptr() as *const std::ffi::c_void, dst, buffer_size as usize);
            rc_device.unmap_memory(memory);
        }

        Ok(Self {
            device,
            buffer,
            memory
        })
    }

    pub fn buffer(&self) -> Buffer {
        self.buffer
    }

    fn get_memory_type_index(
        properties: vk::PhysicalDeviceMemoryProperties,
        candidates: u32,
        required_properties: vk::MemoryPropertyFlags
    ) -> Result<usize, VulkanBackendError> {
        let index = properties.memory_types.iter()
            .take(properties.memory_type_count as usize)
            .enumerate()
            .filter_map(| (i, t) |
                if (candidates & (1 << i as u32) != 0)
                    && (t.property_flags & required_properties == required_properties) { 
                    Some(i) 
                } else {
                    None
                }
            )
            .take(1)
            .collect::<Vec<usize>>();

        if index.is_empty() {
            Err(VulkanBackendError::NoSuitableMemoryFound)
        } else {
            Ok(index[0])
        }
    }
}

impl Drop for VertexBuffer {
    fn drop(&mut self) {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            rc_device.destroy_buffer(self.buffer, None);
            rc_device.free_memory(self.memory, None);
        }
    }
}
