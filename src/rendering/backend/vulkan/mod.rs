use vulkano::instance::Instance;
use vulkano::instance::InstanceExtensions;
use vulkano::instance::PhysicalDevice;
use super::Backend;


pub struct VulkanBackend {
    instance: std::sync::Arc<Instance>,
}

impl Backend for VulkanBackend {

}

impl VulkanBackend {
    pub fn new() -> VulkanBackend {
        let instance = Instance::new(None, &InstanceExtensions::none(), None).unwrap();
        let physical_devices = vulkano::instance::PhysicalDevice::enumerate(&instance);
        for phy_dev in physical_devices {
            println!("Device: {} API Version: {}", phy_dev.name(), phy_dev.api_version());
        }

        VulkanBackend { instance }
    }
}
