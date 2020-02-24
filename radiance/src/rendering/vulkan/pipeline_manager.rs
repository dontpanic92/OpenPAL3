use std::error::Error;
use ash::vk;
use ash::Device;
use super::pipeline::Pipeline;
use super::material::VulkanMaterial;

pub struct PipelineManager {
    pipelines: Vec<vk::Pipeline>,
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {
            pipelines: vec![],
        }
    }

    pub fn get_pipeline(material: &VulkanMaterial) -> Pipeline {

    }

    
}
