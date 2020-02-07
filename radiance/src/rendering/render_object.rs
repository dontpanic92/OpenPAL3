use super::Vertex;
use std::path::PathBuf;

pub struct RenderObject {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    texture_path: PathBuf,
}

impl RenderObject {
    pub fn new() -> Self {
        let mut path = std::env::current_exe().unwrap();
        path.pop();
        path.push("data/test.jpg");
        Self {
            vertices: vec![],
            indices: vec![],
            texture_path: path,
        }
    }

    pub fn new_with_data(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let mut path = std::env::current_exe().unwrap();
        path.pop();
        path.push("data/test.jpg");
        Self {
            vertices,
            indices,
            texture_path: path,
        }
    }

    pub fn vertices(&self) -> &Vec<Vertex> {
        &self.vertices
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }

    pub fn texture_path(&self) -> &PathBuf {
        &self.texture_path
    }
}
