use ash;

#[cfg(target_os = "windows")]
pub fn instance_extension_names() -> Vec<*const i8> {
    vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::Win32Surface::name().as_ptr(),
    ]
}

pub fn device_extension_names() -> Vec<*const i8> {
    vec![ash::extensions::khr::Swapchain::name().as_ptr()]
}
