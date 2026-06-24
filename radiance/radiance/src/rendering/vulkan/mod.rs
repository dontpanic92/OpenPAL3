mod adhoc_command_runner;
mod buffer;
mod creation_helpers;
mod descriptor_managers;
mod descriptor_pool;
mod descriptor_set_layout;
mod device;
mod error;
mod factory;
mod helpers;
mod image;
mod image_view;
mod imgui;
mod instance;
mod material;
mod pipeline;
mod pipeline_layout;
mod pipeline_manager;
mod render_object;
mod render_pass;
mod render_target;
mod sampler;
mod shader;
mod shader_cache;
mod shadow_map;
mod swapchain;
mod texture;
mod uniform_buffers;
mod vulkan_engine;

pub use vulkan_engine::VulkanRenderingEngine;

// Backend-typed handles surfaced to sibling rendering modules
// (rendering_component, render_object, render_target) so they can store
// concrete Vulkan references alongside the cross-backend trait objects
// without paying a per-frame downcast.
pub(super) use render_object::VulkanRenderObject;
pub(super) use render_target::VulkanRenderTarget;
