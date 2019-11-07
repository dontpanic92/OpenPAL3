use super::Engine;
use super::backend::Backend;
use super::backend::vulkan::VulkanBackend;

pub struct Radiance
{
    backend: Box<dyn Backend>,
}

impl Engine for Radiance {

}

impl Radiance {
    pub fn new() -> Radiance {
        let vulkan = Box::new(VulkanBackend::new());
        Radiance { backend: vulkan }
    }
}
