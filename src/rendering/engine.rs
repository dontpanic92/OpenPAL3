use crate::rendering::Vertex;
use crate::math::*;
use super::Engine;
use super::backend::Backend;
use super::backend::vulkan::VulkanBackend;

pub struct RuntimeEngine
{
    backend: Box<dyn Backend>,
}

lazy_static! { 
    pub static ref vertices: Vec<Vertex> = {
        vec![
            Vertex::new(Vec2::new(0f32, -0.5f32), Vec3::new(1f32, 1f32, 0f32)),
            Vertex::new(Vec2::new(0.5f32, 0.5f32), Vec3::new(0f32, 1f32, 0f32)),
            Vertex::new(Vec2::new(-0.5f32, 0.5f32), Vec3::new(0f32, 0f32, 1f32)),
        ]
    };
}

impl Engine for RuntimeEngine {
    fn render(&mut self) {        
        let _ = self.backend.test(&vertices);
    }
}

impl RuntimeEngine {
    pub fn new(window: &super::Window) -> Result<RuntimeEngine, Box<dyn std::error::Error>> {
        let vulkan = VulkanBackend::new(window)?;
        Ok(Self { backend: Box::new(vulkan) })
    }
}
