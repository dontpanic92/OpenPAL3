use super::Engine;
use super::backend::Backend;
use super::backend::vulkan::VulkanBackend;

pub struct RuntimeEngine
{
    backend: Box<dyn Backend>,
}

impl Engine for RuntimeEngine {
    fn render(&mut self) {
        self.backend.test();
    }
}

impl RuntimeEngine {
    pub fn new(window: &super::Window) -> Result<RuntimeEngine, Box<dyn std::error::Error>> {
        let vulkan = VulkanBackend::new(window)?;
        Ok(Self { backend: Box::new(vulkan) })
    }
}


