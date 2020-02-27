use super::material::VulkanMaterial;
use super::{
    pipeline::Pipeline, pipeline_layout::PipelineLayout, render_pass::RenderPass,
    shader::VulkanShader,
};
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
    pipeline_layout: PipelineLayout,
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
        let pipeline_layout = PipelineLayout::new(device, descriptor_manager);

        Self {
            device: Rc::downgrade(device),
            descriptor_manager: Rc::downgrade(descriptor_manager),
            color_format,
            depth_format,
            extent,
            render_pass,
            pipeline_layout,
            pipelines: HashMap::new(),
        }
    }

    pub fn create_pipeline_if_not_exist(&mut self, shader: &VulkanShader) -> &Pipeline {
        let name = shader.name();
        let device = self.device.upgrade().unwrap();
        if !self.pipelines.contains_key(name) {
            println!("not exist: {}", name);
            self.pipelines.insert(
                name.clone(),
                Pipeline::new(
                    &device,
                    &self.pipeline_layout,
                    &self.render_pass,
                    shader,
                    self.extent,
                ),
            );
        }

        self.pipelines.get(name).unwrap()
    }

    pub fn get_pipeline(&mut self, shader_name: &String) -> &Pipeline {
        self.pipelines.get(shader_name).unwrap()
    }

    pub fn render_pass(&self) -> &RenderPass {
        &self.render_pass
    }

    pub fn pipeline_layout(&self) -> &PipelineLayout {
        &self.pipeline_layout
    }
}
