use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::Rc;
use std::rc::Weak;

const MAX_DESCRIPTOR_SET_COUNT: u32 = 40960;

pub struct DescriptorPoolCreateInfo {
    pub ty: vk::DescriptorType,
    pub descriptor_count: u32,
}

pub struct DescriptorPool {
    device: Weak<Device>,
    pool: vk::DescriptorPool,
}

impl DescriptorPool {
    pub fn new(device: &Rc<Device>, create_info: &[DescriptorPoolCreateInfo]) -> Self {
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

        let pool = unsafe { device.create_descriptor_pool(&create_info, None).unwrap() };

        Self {
            device: Rc::downgrade(device),
            pool,
        }
    }

    pub fn reset(&self) {
        let device = self.device.upgrade().unwrap();
        let _ = unsafe {
            device.reset_descriptor_pool(self.pool, vk::DescriptorPoolResetFlags::empty())
        };
    }

    pub fn vk_pool(&self) -> vk::DescriptorPool {
        self.pool
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
