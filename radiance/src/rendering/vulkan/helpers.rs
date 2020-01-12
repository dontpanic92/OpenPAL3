use ash;
use ash::vk;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

#[cfg(target_os = "windows")]
pub fn instance_extension_names() -> Vec<*const i8> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::Win32Surface::name().as_ptr(),
        ash::extensions::ext::DebugReport::name().as_ptr(),
        ash::extensions::ext::DebugUtils::name().as_ptr(),
    ]
}

pub fn device_extension_names() -> Vec<*const i8> {
    vec![ash::extensions::khr::Swapchain::name().as_ptr()]
}

pub unsafe extern "system" fn debug_callback(
    _: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _: u64,
    _: usize,
    _: i32,
    _: *const c_char,
    p_message: *const c_char,
    _: *mut c_void,
) -> u32 {
    println!("{:?}", CStr::from_ptr(p_message));
    vk::FALSE
}
