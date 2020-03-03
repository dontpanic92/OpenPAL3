use super::material::VulkanMaterial;
use super::{pipeline::Pipeline, render_pass::RenderPass};
use crate::rendering::vulkan::descriptor_manager::DescriptorManager;
use ash::vk;
use ash::Device;
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

pub struct PipelineManager {
    device: Weak<Device>,
    descriptor_manager: Weak<DescriptorManager>,
    color_format: vk::Format,
    depth_format: vk::Format,
    extent: vk::Extent2D,
    render_pass: RenderPass,
    pipelines: HashMap<String, Pipeline>,
}

impl PipelineManager {
    pub fn new(
        device: &Rc<Device>,
        descriptor_manager: &Rc<DescriptorManager>,
        color_format: vk::Format,
        depth_format: vk::Format,
        extent: vk::Extent2D,
    ) -> Self {
        let render_pass = RenderPass::new(device, color_format, depth_format);

        Self {
            device: Rc::downgrade(device),
            descriptor_manager: Rc::downgrade(descriptor_manager),
            color_format,
            depth_format,
            extent,
            render_pass,
            pipelines: HashMap::new(),
        }
    }

    pub fn create_pipeline_if_not_exist(&mut self, material: &VulkanMaterial) -> &Pipeline {
        let name = material.name();
        let device = self.device.upgrade().unwrap();
        let descriptor_manager = self.descriptor_manager.upgrade().unwrap();
        if !self.pipelines.contains_key(name) {
            self.pipelines.insert(
                name.to_owned(),
                Pipeline::new(
                    &device,
                    &descriptor_manager,
                    &self.render_pass,
                    material,
                    self.extent,
                ),
            );
        }

        self.pipelines.get(name).unwrap()
    }

    pub fn get_pipeline(&mut self, material_name: &str) -> &Pipeline {
        self.pipelines.get(material_name).unwrap()
    }

    pub fn render_pass(&self) -> &RenderPass {
        &self.render_pass
    }
}
