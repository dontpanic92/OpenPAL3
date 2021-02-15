use super::creation_helpers;
use ash::{
    version::InstanceV1_0,
    vk::{PhysicalDevice, PhysicalDeviceProperties},
    Entry,
};
use std::rc::Rc;

pub struct Instance {
    entry: Rc<Entry>,
    instance: ash::Instance,
}

impl Instance {
    pub fn new(entry: Rc<Entry>) -> Self {
        let instance = creation_helpers::create_instance(&entry).unwrap();
        Self { entry, instance }
    }

    pub fn vk_instance(&self) -> &ash::Instance {
        &self.instance
    }

    pub fn get_physical_device_properties(
        &self,
        physical_device: PhysicalDevice,
    ) -> PhysicalDeviceProperties {
        unsafe {
            self.instance
                .get_physical_device_properties(physical_device)
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}
