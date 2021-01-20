use super::error::VulkanBackendError;
use super::helpers;
use super::image_view::ImageView;
use crate::constants;
use crate::rendering::Window;
use ash::extensions::khr::{Surface, Swapchain};
use ash::prelude::VkResult;
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::vk::{
    Extent2D, PhysicalDevice, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR,
    SwapchainKHR,
};
use ash::{vk, Device, Entry, Instance, InstanceError};
use std::error::Error;
use std::ffi::CString;
use std::rc::Rc;

pub fn create_instance(entry: &Entry) -> Result<Instance, InstanceError> {
    let app_info = vk::ApplicationInfo::builder()
        .engine_name(&CString::new(constants::STR_ENGINE_NAME).unwrap())
        .build();
    let extension_names = helpers::instance_extension_names();
    let layer_names = enabled_layer_names();
    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names)
        .enabled_layer_names(&layer_names)
        .build();
    unsafe { entry.create_instance(&create_info, None) }
}

pub fn get_physical_device(instance: &Instance) -> Result<PhysicalDevice, Box<dyn Error>> {
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };
    if physical_devices.is_empty() {
        return Err(VulkanBackendError::NoVulkanDeviceFound)?;
    }

    Ok(physical_devices[0])
}

pub fn create_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> VkResult<vk::SurfaceKHR> {
    let win32surface_entry = ash::extensions::khr::Win32Surface::new(entry, instance);
    let instance = unsafe { winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null_mut()) };
    let create_info = vk::Win32SurfaceCreateInfoKHR::builder()
        .hinstance(instance as *const std::ffi::c_void)
        .hwnd(window.hwnd as *const std::ffi::c_void)
        .build();
    unsafe { win32surface_entry.create_win32_surface(&create_info, None) }
}

pub fn get_graphics_queue_family_index(
    instance: &Instance,
    physical_device: PhysicalDevice,
    surface_entry: &Surface,
    surface: SurfaceKHR,
) -> Result<u32, VulkanBackendError> {
    let queue_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    queue_properties
        .iter()
        .enumerate()
        .position(|(i, &x)| {
            x.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && unsafe {
                    surface_entry
                        .get_physical_device_surface_support(physical_device, i as u32, surface)
                        .unwrap()
                }
        })
        .map(|f| f as u32)
        .ok_or(VulkanBackendError::NoGraphicQueueFound)
}

pub fn create_device(
    instance: &Instance,
    physical_device: PhysicalDevice,
    graphic_queue_family_index: u32,
) -> VkResult<Device> {
    let priorities = [0.5 as f32];
    let queue_create_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(graphic_queue_family_index)
        .queue_priorities(&priorities)
        .build();
    let extension_names = helpers::device_extension_names();
    let queue_create_info = [queue_create_info];
    let physical_device_features = vk::PhysicalDeviceFeatures::builder()
        .sampler_anisotropy(true)
        .build();
    let create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_info)
        .enabled_extension_names(&extension_names)
        .enabled_features(&physical_device_features)
        .build();
    unsafe { instance.create_device(physical_device, &create_info, None) }
}

pub fn get_surface_format(
    physical_device: PhysicalDevice,
    surface_entry: &Surface,
    surface: SurfaceKHR,
) -> Result<SurfaceFormatKHR, Box<dyn Error>> {
    let formats =
        unsafe { surface_entry.get_physical_device_surface_formats(physical_device, surface)? };

    if formats.len() == 0 {
        return Err(VulkanBackendError::NoSurfaceFormatSupported)?;
    }

    let default = formats[0];
    Ok(formats
        .into_iter()
        .find(|f| {
            f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                && f.format == vk::Format::R8G8B8A8_UNORM
        })
        .unwrap_or(default))
}

pub fn get_present_mode(
    physical_device: PhysicalDevice,
    surface_entry: &Surface,
    surface: SurfaceKHR,
) -> Result<PresentModeKHR, Box<dyn Error>> {
    let present_modes = unsafe {
        surface_entry.get_physical_device_surface_present_modes(physical_device, surface)?
    };

    if present_modes.len() == 0 {
        return Err(VulkanBackendError::NoSurfacePresentModeSupported)?;
    }

    Ok(present_modes
        .into_iter()
        .find(|f| f == &vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO))
}

pub fn create_swapchain(
    swapchain_entry: &Swapchain,
    surface: SurfaceKHR,
    capabilities: SurfaceCapabilitiesKHR,
    format: SurfaceFormatKHR,
    present_mode: PresentModeKHR,
) -> VkResult<SwapchainKHR> {
    let create_info = vk::SwapchainCreateInfoKHR::builder()
        .surface(surface)
        .min_image_count(capabilities.min_image_count + 1)
        .image_format(format.format)
        .image_color_space(format.color_space)
        .image_array_layers(1)
        .image_extent(capabilities.current_extent)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(vk::SwapchainKHR::default())
        .build();
    unsafe { swapchain_entry.create_swapchain(&create_info, None) }
}

pub fn create_image_views(
    device: &Rc<super::device::Device>,
    images: &Vec<vk::Image>,
    format: SurfaceFormatKHR,
) -> VkResult<Vec<ImageView>> {
    images
        .iter()
        .map(|image| ImageView::new_color_image_view(device.clone(), *image, format.format))
        .collect()
}

pub fn create_shader_module(
    device: &Device,
    shader_path: &str,
) -> Result<vk::ShaderModule, Box<dyn Error>> {
    let code = std::fs::read(shader_path)?;
    let code_u32 =
        unsafe { std::slice::from_raw_parts::<u32>(code.as_ptr().cast(), code.len() / 4) };
    let create_info = vk::ShaderModuleCreateInfo::builder().code(code_u32).build();
    unsafe { Ok(device.create_shader_module(&create_info, None)?) }
}

pub fn create_framebuffers(
    device: &super::device::Device,
    image_views: &Vec<ImageView>,
    depth_image_view: &ImageView,
    extent: &Extent2D,
    render_pass: vk::RenderPass,
) -> VkResult<Vec<vk::Framebuffer>> {
    image_views
        .iter()
        .map(|view| {
            let attachments = [view.vk_image_view(), depth_image_view.vk_image_view()];
            let create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&attachments)
                .layers(1)
                .width(extent.width)
                .height(extent.height)
                .build();
            device.create_framebuffer(&create_info)
        })
        .collect()
}

fn enabled_layer_names() -> Vec<*const i8> {
    unsafe {
        vec![
            // Use $env:VK_INSTANCE_LAYERS="VK_LAYER_LUNARG_standard_validation" to enable the validation layer
            // instead of doing so here.
            //
            // std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_LUNARG_standard_validation\0")
            //     .as_ptr() as *const i8,
        ]
    }
}
