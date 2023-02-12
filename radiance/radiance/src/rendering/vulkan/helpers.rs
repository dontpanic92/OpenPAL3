use ash;
use ash::vk;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

#[cfg(target_os = "macos")]
pub fn instance_extension_names() -> Vec<*const c_char> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::ext::MetalSurface::name().as_ptr(),
        ash::extensions::ext::DebugUtils::name().as_ptr(),
        ash::vk::KhrPortabilityEnumerationFn::name().as_ptr(),
        ash::vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr(),
    ]
}
#[cfg(target_os = "linux")]
pub fn instance_extension_names() -> Vec<*const c_char> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::XlibSurface::name().as_ptr(),
        ash::extensions::ext::DebugUtils::name().as_ptr(),
    ]
}
#[cfg(target_os = "android")]
pub fn instance_extension_names() -> Vec<*const c_char> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::AndroidSurface::name().as_ptr(),
    ]
}
#[cfg(target_os = "windows")]
pub fn instance_extension_names() -> Vec<*const i8> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::Win32Surface::name().as_ptr(),
        ash::extensions::ext::DebugUtils::name().as_ptr(),
    ]
}

pub fn device_extension_names() -> Vec<*const c_char> {
    vec![ash::extensions::khr::Swapchain::name().as_ptr()]
}

pub unsafe extern "system" fn debug_callback(
    _message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let message = CStr::from_ptr((*p_callback_data).p_message);
    println!("validation layer: {:?}", message);

    vk::FALSE
}
