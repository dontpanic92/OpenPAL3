use super::Vertex;
use crate::math::*;

pub struct RenderObject {}

impl RenderObject {
    pub fn new() -> Self {
        Self {}
    }

    pub fn vertices(&self) -> Vec<Vertex> {
        vec![
            Vertex::new(Vec2::new(-0.5f32, -0.5f32), Vec3::new(1f32, 0f32, 0f32)),
            Vertex::new(Vec2::new(0.5f32, -0.5f32), Vec3::new(0f32, 1f32, 0f32)),
            Vertex::new(Vec2::new(0.5f32, 0.5f32), Vec3::new(0f32, 0f32, 1f32)),
            Vertex::new(Vec2::new(-0.5f32, 0.5f32), Vec3::new(1f32, 1f32, 1f32)),
        ]
    }

    pub fn indices(&self) -> Vec<u32> {
        vec![0, 1, 2, 2, 3, 0]
    }
}
