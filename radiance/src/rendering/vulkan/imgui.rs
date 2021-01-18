use ash::{vk, Device, Instance};
use imgui::*;
use imgui_rs_vulkan_renderer::*;
use std::rc::Rc;

use crate::imgui::{ImguiContext, ImguiFrame};

pub struct ImguiVulkanContext {
    renderer: Renderer,
    vk_context: VkContext,
}

impl ImguiVulkanContext {
    pub fn new(
        instance: &Rc<Instance>,
        physical_device: vk::PhysicalDevice,
        device: &Rc<Device>,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
        in_flight_frames: usize,
        context: &mut ImguiContext,
    ) -> Self {
        let vk_context = VkContext {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            queue,
            command_pool,
        };

        let renderer = Renderer::new(
            &vk_context,
            in_flight_frames,
            render_pass,
            &mut context.context_mut(),
        )
        .unwrap();

        Self {
            renderer,
            vk_context,
        }
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
            self.renderer
                .cmd_draw(&self.vk_context, command_buffer, draw_data)
                .unwrap();
        }

        unsafe {
            sys::igEndFrame();
        }
    }
}

impl Drop for ImguiVulkanContext {
    fn drop(&mut self) {
        self.renderer.destroy(&self.vk_context).unwrap();
    }
}

struct VkContext {
    instance: Rc<Instance>,
    device: Rc<Device>,
    physical_device: vk::PhysicalDevice,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
}

impl RendererVkContext for VkContext {
    fn instance(&self) -> &Instance {
        self.instance.as_ref()
    }

    fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    fn device(&self) -> &Device {
        self.device.as_ref()
    }

    fn queue(&self) -> vk::Queue {
        self.queue
    }

    fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }
}
