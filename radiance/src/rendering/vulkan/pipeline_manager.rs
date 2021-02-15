use super::{device::Device, material::VulkanMaterial};
use super::{pipeline::Pipeline, render_pass::RenderPass};
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use ash::vk;
use std::collections::HashMap;
use std::rc::Rc;

pub struct PipelineManager {
    device: Rc<Device>,
    descriptor_manager: Rc<DescriptorManager>,
    color_format: vk::Format,
    depth_format: vk::Format,
    extent: vk::Extent2D,
    render_pass: RenderPass,
    pipelines: HashMap<String, Pipeline>,
}

impl PipelineManager {
    pub fn new(
        device: Rc<Device>,
        descriptor_manager: &Rc<DescriptorManager>,
        color_format: vk::Format,
        depth_format: vk::Format,
        extent: vk::Extent2D,
    ) -> Self {
        let render_pass = RenderPass::new(device.clone(), color_format, depth_format);

        Self {
            device,
            descriptor_manager: descriptor_manager.clone(),
            color_format,
            depth_format,
            extent,
            render_pass,
            pipelines: HashMap::new(),
        }
    }

    pub fn create_pipeline_if_not_exist(&mut self, material: &VulkanMaterial) -> &Pipeline {
        let name = material.name();
        if !self.pipelines.contains_key(name) {
            self.pipelines.insert(
                name.to_owned(),
                Pipeline::new(
                    self.device.clone(),
                    &self.descriptor_manager,
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
