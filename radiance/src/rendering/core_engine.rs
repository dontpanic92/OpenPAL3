use super::backend::RenderingBackend;
use super::RenderingEngine;
use crate::math::*;
use crate::rendering::Vertex;

pub struct CoreRenderingEngine<TBackend: RenderingBackend> {
    backend: TBackend,
}

lazy_static! {
    pub static ref vertices: Vec<Vertex> = {
        vec![
            Vertex::new(Vec2::new(-0.5f32, -0.5f32), Vec3::new(1f32, 0f32, 0f32)),
            Vertex::new(Vec2::new(0.5f32, -0.5f32), Vec3::new(0f32, 1f32, 0f32)),
            Vertex::new(Vec2::new(0.5f32, 0.5f32), Vec3::new(0f32, 0f32, 1f32)),
            Vertex::new(Vec2::new(-0.5f32, 0.5f32), Vec3::new(1f32, 1f32, 1f32)),
        ]
    };
    pub static ref indices: Vec<u32> = { vec![0, 1, 2, 2, 3, 0] };
}

impl<TBackend: RenderingBackend> RenderingEngine for CoreRenderingEngine<TBackend> {
    fn render(&mut self) {
        let _ = self.backend.test();
    }
}

impl<TBackend: RenderingBackend> CoreRenderingEngine<TBackend> {
    pub fn new(
        window: &super::Window,
    ) -> Result<CoreRenderingEngine<TBackend>, Box<dyn std::error::Error>> {
        let vulkan = TBackend::new(window)?;
        Ok(Self { backend: vulkan })
    }
}
