use super::error::VulkanBackendError;
use super::helpers;
use crate::constants;
use crate::rendering::Window;
use ash::extensions::khr::{Surface, Swapchain};
use ash::prelude::VkResult;
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::vk::{
    Extent2D, Image, ImageView, Offset2D, PhysicalDevice, Pipeline, PresentModeKHR,
    ShaderStageFlags, SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR, SwapchainKHR,
};
use ash::{vk, Device, Entry, Instance, InstanceError};
use std::error::Error;
use std::ffi::CString;

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
                    surface_entry.get_physical_device_surface_support(
                        physical_device,
                        i as u32,
                        surface,
                    )
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
    let queue_create_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(graphic_queue_family_index)
        .queue_priorities(&[0.5 as f32])
        .build();
    let extension_names = helpers::device_extension_names();
    let create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&[queue_create_info])
        .enabled_extension_names(&extension_names)
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
    device: &Device,
    images: &Vec<Image>,
    format: SurfaceFormatKHR,
) -> VkResult<Vec<ImageView>> {
    let component_mapping = vk::ComponentMapping::builder()
        .a(vk::ComponentSwizzle::IDENTITY)
        .r(vk::ComponentSwizzle::IDENTITY)
        .g(vk::ComponentSwizzle::IDENTITY)
        .b(vk::ComponentSwizzle::IDENTITY)
        .build();
    let subres_range = vk::ImageSubresourceRange::builder()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_array_layer(0)
        .layer_count(1)
        .base_mip_level(0)
        .level_count(1)
        .build();
    images
        .iter()
        .map(|f| {
            let create_info = vk::ImageViewCreateInfo::builder()
                .format(format.format)
                .image(*f)
                .components(component_mapping)
                .subresource_range(subres_range)
                .build();
            unsafe { device.create_image_view(&create_info, None) }
        })
        .collect()
}

pub fn create_pipeline_layout(device: &Device) -> VkResult<vk::PipelineLayout> {
    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder().build();
    unsafe { device.create_pipeline_layout(&pipeline_layout_create_info, None) }
}

pub fn create_render_pass(device: &Device, format: SurfaceFormatKHR) -> VkResult<vk::RenderPass> {
    let attachment_description = vk::AttachmentDescription::builder()
        .format(format.format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .build();

    let attachment_reference = vk::AttachmentReference::builder()
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build();

    let subpass_description = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&[attachment_reference])
        .build();

    let subpass_dependency = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::default())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )
        .build();

    let render_pass_create_info = vk::RenderPassCreateInfo::builder()
        .attachments(&[attachment_description])
        .subpasses(&[subpass_description])
        .dependencies(&[subpass_dependency])
        .build();

    unsafe { device.create_render_pass(&render_pass_create_info, None) }
}

static SIMPLE_TRIANGLE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.vert.spv"));
static SIMPLE_TRIANGLE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/simple_triangle.frag.spv"));

pub fn create_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    extent: &Extent2D,
) -> Result<Vec<Pipeline>, Box<dyn Error>> {
    let _current_exe = std::env::current_exe()?;
    let vert_shader = create_shader_module_from_array(device, SIMPLE_TRIANGLE_VERT)?;
    let frag_shader = create_shader_module_from_array(device, SIMPLE_TRIANGLE_FRAG)?;

    let entry_point = &CString::new("main").unwrap();
    let vert_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
        .name(entry_point)
        .stage(ShaderStageFlags::VERTEX)
        .module(vert_shader)
        .build();
    let frag_shader_stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
        .name(entry_point)
        .stage(ShaderStageFlags::FRAGMENT)
        .module(frag_shader)
        .build();

    let pipeline_vertex_input_create_info = {
        let binding_descriptions = [super::vertex_helper::get_binding_description()];
        let attribute_desctiprions = super::vertex_helper::get_attribute_descriptions();
        vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&attribute_desctiprions)
            .vertex_binding_descriptions(&binding_descriptions)
            .build()
    };

    let pipeline_input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false)
        .build();

    let viewport = vk::Viewport::builder()
        .x(0f32)
        .y(0f32)
        .min_depth(0f32)
        .max_depth(1f32)
        .height(extent.height as f32)
        .width(extent.width as f32)
        .build();

    let scissor = vk::Rect2D::builder()
        .extent(*extent)
        .offset(Offset2D::builder().x(0).y(0).build())
        .build();

    let pipeline_viewport_state_create_info = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(&[viewport])
        .scissors(&[scissor])
        .build();

    let pipeline_rasterization_state_create_info =
        vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1f32)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false)
            .build();

    let pipeline_multisamle_state_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .build();

    let pipeline_color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        )
        .blend_enable(false)
        .build();

    let pipeline_color_blending_state_create_info =
        vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(&[pipeline_color_blend_attachment_state])
            .build();

    let create_info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(&[vert_shader_stage_create_info, frag_shader_stage_create_info])
        .vertex_input_state(&pipeline_vertex_input_create_info)
        .input_assembly_state(&pipeline_input_assembly_create_info)
        .viewport_state(&pipeline_viewport_state_create_info)
        .rasterization_state(&pipeline_rasterization_state_create_info)
        .multisample_state(&pipeline_multisamle_state_create_info)
        .color_blend_state(&pipeline_color_blending_state_create_info)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0)
        .build();

    let pipelines = match unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::default(), &[create_info], None)
    } {
        Ok(p) => Ok(p),
        Err((p, e)) => {
            for x in p.into_iter() {
                unsafe { device.destroy_pipeline(x, None) };
            }

            Err(Box::new(e) as Box<dyn Error>)
        }
    };

    unsafe {
        device.destroy_shader_module(vert_shader, None);
        device.destroy_shader_module(frag_shader, None);
    }

    return pipelines;
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

fn create_shader_module_from_array(
    device: &Device,
    code: &[u8],
) -> Result<vk::ShaderModule, Box<dyn Error>> {
    let code_u32 =
        unsafe { std::slice::from_raw_parts::<u32>(code.as_ptr().cast(), code.len() / 4) };
    let create_info = vk::ShaderModuleCreateInfo::builder().code(code_u32).build();
    unsafe { Ok(device.create_shader_module(&create_info, None)?) }
}

pub fn create_framebuffers(
    device: &Device,
    image_views: &Vec<ImageView>,
    extent: &Extent2D,
    render_pass: vk::RenderPass,
) -> VkResult<Vec<vk::Framebuffer>> {
    image_views
        .iter()
        .map(|view| {
            let create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&[*view])
                .layers(1)
                .width(extent.width)
                .height(extent.height)
                .build();
            unsafe { device.create_framebuffer(&create_info, None) }
        })
        .collect()
}

fn enabled_layer_names() -> Vec<*const i8> {
    unsafe {
        vec![
            std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_LUNARG_standard_validation\0")
                .as_ptr() as *const i8,
        ]
    }
}
