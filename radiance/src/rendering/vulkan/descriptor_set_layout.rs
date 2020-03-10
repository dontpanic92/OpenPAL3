use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::Rc;
use std::rc::Weak;

pub struct DescriptorSetLayout {
    device: Weak<Device>,
    layout: vk::DescriptorSetLayout,
}

impl DescriptorSetLayout {
    pub fn new(
        device: &Rc<Device>,
        descriptor_type: vk::DescriptorType,
        stage_flags: vk::ShaderStageFlags,
        descriptor_count: u32,
    ) -> Self {
        let layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(descriptor_count)
            .descriptor_type(descriptor_type)
            .stage_flags(stage_flags)
            .build();

        let bindings = [layout_binding];
        let create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();

        let layout = unsafe { device.create_descriptor_set_layout(&create_info, None) }.unwrap();
        Self {
            device: Rc::downgrade(device),
            layout,
        }
    }

    pub fn vk_layout(&self) -> vk::DescriptorSetLayout {
        self.layout
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_descriptor_set_layout(self.layout, None);
        }
    }
}
