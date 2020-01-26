use super::Vertex;
use crate::math::*;

pub struct RenderObject {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl RenderObject {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            indices: vec![],
        }
    }

    pub fn new_with_data(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertices(&self) -> &Vec<Vertex> {
        &self.vertices
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }
}
