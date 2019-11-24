use crate::constants;
use std::fmt;

#[derive(Debug)]
pub enum VulkanBackendError {
    NoVulkanDeviceFound,
    NoGraphicQueueFound,
}

impl fmt::Display for VulkanBackendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VulkanBackendError::NoVulkanDeviceFound => write!(f, "{}", constants::STR_ZERO_VULKAN_PHYSICAL_DEVICE),
            VulkanBackendError::NoGraphicQueueFound => write!(f, "{}", constants::STR_ZERO_VULKAN_GRAPHICS_QUEUE),
        }
    }
}

impl std::error::Error for VulkanBackendError {}
