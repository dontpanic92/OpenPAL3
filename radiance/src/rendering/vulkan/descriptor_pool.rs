use ash::vk;
use std::rc::Rc;

use super::device::Device;

const MAX_DESCRIPTOR_SET_COUNT: u32 = 40960;

pub struct DescriptorPoolCreateInfo {
    pub ty: vk::DescriptorType,
    pub descriptor_count: u32,
}

pub struct DescriptorPool {
    device: Rc<Device>,
    pool: vk::DescriptorPool,
}

impl DescriptorPool {
    pub fn new(device: Rc<Device>, create_info: &[DescriptorPoolCreateInfo]) -> Self {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = create_info
            .iter()
            .map(|info| {
                vk::DescriptorPoolSize::builder()
                    .descriptor_count(info.descriptor_count)
                    .ty(info.ty)
                    .build()
            })
            .collect();

        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_DESCRIPTOR_SET_COUNT)
            .build();

        let pool = device.create_descriptor_pool(&create_info).unwrap();

        Self { device, pool }
    }

    pub fn reset(&self) {
        self.device.reset_descriptor_pool(self.pool).unwrap();
    }

    pub fn vk_pool(&self) -> vk::DescriptorPool {
        self.pool
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        self.device.destroy_descriptor_pool(self.pool);
    }
}
