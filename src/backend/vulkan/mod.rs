use vulkano::instance::Instance;
use vulkano::instance::InstanceExtensions;
use super::Backend;


pub struct VulkanBackend {
    instance: std::sync::Arc<Instance>,
}

impl Backend for VulkanBackend {
    fn new() -> VulkanBackend {
        let instance = dbg!(Instance::new(None, &InstanceExtensions::none(), None))
            .expect("Failed to create instance");

        VulkanBackend { instance }
        
    }
}
