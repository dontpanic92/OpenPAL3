use ash;

#[cfg(target_os = "windows")]
pub fn surface_extension_names() -> Vec<*const i8>
{
    vec![ash::extensions::khr::Surface::name().as_ptr(), ash::extensions::khr::Win32Surface::name().as_ptr()]
}