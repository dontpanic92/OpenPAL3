use super::error::VulkanBackendError;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::{vk, Device, Instance};
use std::rc::{Rc, Weak};

pub struct Memory {
    device: Weak<Device>,
    device_memory: vk::DeviceMemory,
    size: u64,
}

impl Memory {
    pub fn new_for_buffer(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        buffer: vk::Buffer,
        memory_prop_flags: vk::MemoryPropertyFlags,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        Memory::new(
            instance,
            device,
            physical_device,
            requirements,
            memory_prop_flags,
        )
    }

    pub fn new_for_image(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        image: vk::Image,
        memory_prop_flags: vk::MemoryPropertyFlags,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let requirements = unsafe { device.get_image_memory_requirements(image) };
        Memory::new(
            instance,
            device,
            physical_device,
            requirements,
            memory_prop_flags,
        )
    }

    pub fn new(
        instance: &Instance,
        device: &Rc<Device>,
        physical_device: vk::PhysicalDevice,
        requirements: vk::MemoryRequirements,
        memory_prop_flags: vk::MemoryPropertyFlags,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let index = Memory::get_memory_type_index(
            properties,
            requirements.memory_type_bits,
            memory_prop_flags,
        )?;

        let create_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(index as u32)
            .build();

        let memory = unsafe { device.allocate_memory(&create_info, None)? };
        Ok(Self {
            device: Rc::downgrade(device),
            device_memory: memory,
            size: requirements.size,
        })
    }

    pub fn copy_from<T>(&mut self, data: &[T]) -> ash::prelude::VkResult<()> {
        let rc_device = self.device.upgrade().unwrap();
        unsafe {
            let dst = rc_device.map_memory(
                self.device_memory,
                0,
                self.size,
                vk::MemoryMapFlags::default(),
            )?;
            std::ptr::copy(
                data.as_ptr() as *const std::ffi::c_void,
                dst,
                self.size as usize,
            );
            rc_device.unmap_memory(self.device_memory);
        }

        Ok(())
    }

    pub fn vk_device_memory(&self) -> vk::DeviceMemory {
        self.device_memory
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

impl Drop for Memory {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.free_memory(self.device_memory, None);
        }
    }
}
