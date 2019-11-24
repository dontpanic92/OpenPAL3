use super::Backend;
use crate::constants;
use ash::{vk, Entry, Instance, Device};
use ash::vk::{CommandPool};
use ash::version::{EntryV1_0, InstanceV1_0, DeviceV1_0};
use error::VulkanBackendError;
use std::ffi::CString;

mod platform;
mod error;

pub struct VulkanBackend {
    instance: Instance,
    device: Device,
    command_pool: CommandPool,
}

impl Backend for VulkanBackend {
    fn test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let buffers = {
            let create_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(self.command_pool)
                .command_buffer_count(50)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();
            unsafe { self.device.allocate_command_buffers(&create_info)? }
        };

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe {
            self.device.begin_command_buffer(buffers[0], &begin_info);
            self.device.end_command_buffer(buffers[0]);
        }

        unsafe { self.device.free_command_buffers(self.command_pool, &buffers) };
        Ok(())
    }
}

impl VulkanBackend {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::new()?;
        let instance = {
            let app_info = vk::ApplicationInfo::builder()
                .engine_name(&CString::new(constants::STR_ENGINE_NAME).unwrap())
                .build();
            let extension_names = platform::surface_extension_names();
            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&extension_names)
                .build();
            unsafe { entry.create_instance(&create_info, None) }?
        };

        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        if physical_devices.is_empty() {
            return Err(VulkanBackendError::NoVulkanDeviceFound)?;
        }

        let physical_device = physical_devices[0];
        let queue_properties = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let graphic_queue_family_index = queue_properties.iter()
            .position(|&x| x.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or(VulkanBackendError::NoGraphicQueueFound)?
            as u32;

        let device = {
            let queue_create_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(graphic_queue_family_index)
                .queue_priorities(&[0.5 as f32])
                .build();
            let create_info = vk::DeviceCreateInfo::builder().queue_create_infos(&[queue_create_info]).build();
            unsafe { instance.create_device(physical_device, &create_info, None)? }
        };

        let command_pool = {
            let device_queue = unsafe { device.get_device_queue(graphic_queue_family_index, 0) };
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER | vk::CommandPoolCreateFlags::TRANSIENT)
                .build();
            unsafe { device.create_command_pool(&create_info, None)? }
        };

        Ok(Self { instance, device, command_pool })
    }
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle();
            self.device.destroy_command_pool(self.command_pool, None);
            self.instance.destroy_instance(None);
        }
    }
}
