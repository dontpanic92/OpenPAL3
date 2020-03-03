use ash::version::DeviceV1_0;
use ash::{prelude::VkResult, vk, Device};
use std::rc::Rc;
use std::rc::Weak;

pub struct PipelineLayout {
    device: Weak<Device>,
    pipeline_layout: vk::PipelineLayout,
}

impl PipelineLayout {
    pub fn new(device: &Rc<Device>, descriptor_set_layouts: &[vk::DescriptorSetLayout]) -> Self {
        let pipeline_layout = Self::create_pipeline_layout(device, descriptor_set_layouts).unwrap();

        Self {
            device: Rc::downgrade(device),
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
        unsafe { device.create_pipeline_layout(&pipeline_layout_create_info, None) }
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
