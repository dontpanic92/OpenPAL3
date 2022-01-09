use super::{device::Device, instance::Instance};
use crate::imgui::{ImguiContext, ImguiFrame};
use ash::vk::{self, DescriptorSet};
use imgui::*;
use imgui_rs_vulkan_renderer::*;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use vk_mem::{Allocator, AllocatorCreateFlags, AllocatorCreateInfo};

pub struct ImguiRenderer {
    renderer: Renderer,
}

impl ImguiRenderer {
    pub fn new(
        instance: Rc<Instance>,
        physical_device: vk::PhysicalDevice,
        device: Rc<Device>,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
        in_flight_frames: usize,
        context: &mut ImguiContext,
    ) -> Self {
        let renderer = {
            let allocator = {
                let allocator_create_info = AllocatorCreateInfo {
                    physical_device: physical_device,
                    device: device.vk_device().clone(),
                    instance: instance.vk_instance().clone(),
                    flags: AllocatorCreateFlags::NONE,
                    preferred_large_heap_block_size: 0,
                    frame_in_use_count: 0,
                    heap_size_limits: None,
                };
                Allocator::new(&allocator_create_info).unwrap()
            };

            // Texture width desired by user before building the atlas.
            // Must be a power-of-two. If you have many glyphs and your graphics API has texture size
            // restrictions, you may want to increase texture width to decrease the height.
            // For example, Apple's Metal API (MTLTextureDescriptor) only supports
            // a maximum size of 16384 (128x128) for texture dimension on M1 devices.
            // We are defining the max width to be the max image dimension size get from device
            // properties here to be safe.
            if cfg!(target_os = "macos") {
                let device_properties = unsafe {
                    instance
                        .vk_instance()
                        .get_physical_device_properties(physical_device)
                };
                context.context_mut().fonts().tex_desired_width =
                    device_properties.limits.max_image_dimension2_d as i32;
            }

            let options = Some(Options {
                in_flight_frames: 1,
                enable_depth_test: true,
                enable_depth_write: true,
            });

            Renderer::with_vk_mem_allocator(
                Arc::new(Mutex::new(allocator)),
                device.vk_device().clone(),
                queue,
                command_pool,
                render_pass,
                &mut context.context_mut(),
                options,
            )
            .unwrap()
        };

        Self { renderer }
    }

    pub fn upsert_texture(
        &mut self,
        id: Option<TextureId>,
        descriptor_set: DescriptorSet,
    ) -> TextureId {
        if let Some(id) = id {
            self.renderer.textures().replace(id, descriptor_set);
            return id;
        } else {
            self.renderer.textures().insert(descriptor_set)
        }
    }

    pub fn set_render_pass(&mut self, render_pass: vk::RenderPass) -> RendererResult<()> {
        self.renderer.set_render_pass(render_pass)
    }

    pub fn record_command_buffer(&mut self, frame: ImguiFrame, command_buffer: vk::CommandBuffer) {
        if !frame.frame_begun {
            return;
        }

        let draw_data = unsafe {
            sys::igRender();
            &*(sys::igGetDrawData() as *mut DrawData)
        };

        if draw_data.total_idx_count > 0 {
            self.renderer.cmd_draw(command_buffer, draw_data).unwrap();
        }
    }
}
