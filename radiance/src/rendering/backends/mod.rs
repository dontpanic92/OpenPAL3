#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub mod vulkan;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub type DefaultRenderingEngine = vulkan::VulkanRenderingEngine;

#[cfg(target_os = "psp")]
pub mod gu;

#[cfg(target_os = "psp")]
pub type DefaultRenderingEngine = gu::GuRenderingEngine;
