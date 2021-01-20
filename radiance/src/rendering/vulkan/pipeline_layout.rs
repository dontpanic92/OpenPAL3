use ash::{prelude::VkResult, vk};
use std::rc::Rc;

use super::device::Device;

pub struct PipelineLayout {
    device: Rc<Device>,
    pipeline_layout: vk::PipelineLayout,
}

impl PipelineLayout {
    pub fn new(device: Rc<Device>, descriptor_set_layouts: &[vk::DescriptorSetLayout]) -> Self {
        let pipeline_layout =
            Self::create_pipeline_layout(&device, descriptor_set_layouts).unwrap();

        Self {
            device,
            pipeline_layout,
        }
    }

    pub fn vk_pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }

    fn create_pipeline_layout(
        device: &Rc<Device>,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
    ) -> VkResult<vk::PipelineLayout> {
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(descriptor_set_layouts)
            .build();
        device.create_pipeline_layout(&pipeline_layout_create_info)
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        self.device.destroy_pipeline_layout(self.pipeline_layout);
    }
}
