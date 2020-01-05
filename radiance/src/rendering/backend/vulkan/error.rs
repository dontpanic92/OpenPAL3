use crate::constants;
use std::fmt;

#[derive(Debug)]
pub enum VulkanBackendError {
    NoVulkanDeviceFound,
    NoGraphicQueueFound,
    NoSurfaceFormatSupported,
    NoSurfacePresentModeSupported,
    NoSuitableMemoryFound,
}

impl fmt::Display for VulkanBackendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VulkanBackendError::NoVulkanDeviceFound => {
                write!(f, "{}", constants::STR_ZERO_VULKAN_PHYSICAL_DEVICE)
            }
            VulkanBackendError::NoGraphicQueueFound => {
                write!(f, "{}", constants::STR_ZERO_VULKAN_GRAPHICS_QUEUE)
            }
            VulkanBackendError::NoSurfaceFormatSupported => {
                write!(f, "{}", constants::STR_ZERO_VULKAN_SURFACE_FORMAT)
            }
            VulkanBackendError::NoSurfacePresentModeSupported => {
                write!(f, "{}", constants::STR_ZERO_VULKAN_SURFACE_PRESENT_MODE)
            }
            VulkanBackendError::NoSuitableMemoryFound => {
                write!(f, "{}", constants::STR_NO_SUITABLE_MEMORY_TYPE)
            }
        }
    }
}

impl std::error::Error for VulkanBackendError {}
