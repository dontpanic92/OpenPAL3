use super::{device::Device, instance::Instance};
use crate::imgui::{ImguiContext, ImguiFrame};
use ash::vk;
use imgui::*;
use imgui_rs_vulkan_renderer::*;
use std::rc::Rc;

pub struct ImguiVulkanContext {
    renderer: Renderer,
    vk_context: VkContext,
}

impl ImguiVulkanContext {
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
        let vk_context = VkContext {
            instance,
            device,
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
    fn instance(&self) -> &ash::Instance {
        self.instance.vk_instance()
    }

    fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    fn device(&self) -> &ash::Device {
        self.device.vk_device()
    }

    fn queue(&self) -> vk::Queue {
        self.queue
    }

    fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }
}
