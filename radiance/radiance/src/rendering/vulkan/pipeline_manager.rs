use super::{device::Device, material::VulkanMaterial};
use super::{pipeline::Pipeline, render_pass::RenderPass};
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::MaterialKey;
use ash::vk;
use std::collections::HashMap;
use std::rc::Rc;

pub struct PipelineManager {
    device: Rc<Device>,
    descriptor_manager: Rc<DescriptorManager>,
    _color_format: vk::Format,
    _depth_format: vk::Format,
    extent: vk::Extent2D,
    render_pass: RenderPass,
    pipelines: HashMap<MaterialKey, Pipeline>,
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
            _color_format: color_format,
            _depth_format: depth_format,
            extent,
            render_pass,
            pipelines: HashMap::new(),
        }
    }

    pub fn create_pipeline_if_not_exist(&mut self, material: &VulkanMaterial) -> &Pipeline {
        let key = *material.key();
        if !self.pipelines.contains_key(&key) {
            self.pipelines.insert(
                key,
                Pipeline::new(
                    self.device.clone(),
                    &self.descriptor_manager,
                    &self.render_pass,
                    material,
                    self.extent,
                ),
            );
        }

        self.pipelines.get(&key).unwrap()
    }

    pub fn get_pipeline(&mut self, key: &MaterialKey) -> &Pipeline {
        self.pipelines.get(key).unwrap()
    }

    pub fn render_pass(&self) -> &RenderPass {
        &self.render_pass
    }
}
