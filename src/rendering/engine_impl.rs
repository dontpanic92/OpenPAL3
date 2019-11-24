use super::Engine;
use super::backend::Backend;
use super::backend::vulkan::VulkanBackend;

pub struct EngineImpl
{
    backend: Box<dyn Backend>,
}

impl Engine for EngineImpl {
    fn render(&mut self) {
        self.backend.test();
    }
}

impl EngineImpl {
    pub fn new() -> Result<EngineImpl, Box<dyn std::error::Error>> {
        let vulkan = VulkanBackend::new()?;
        Ok(Self { backend: Box::new(vulkan) })
    }
}
