use super::Vertex;
use std::path::PathBuf;

pub struct RenderObject {
    vertices: Vec<Vec<Vertex>>,
    indices: Vec<u32>,
    texture_path: PathBuf,
    anim_frame_index: usize,
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
            anim_frame_index: 0,
        }
    }

    pub fn new_with_data(vertices: Vec<Vec<Vertex>>, indices: Vec<u32>) -> Self {
        let mut path = std::env::current_exe().unwrap();
        path.pop();
        path.push("data/test.jpg");
        Self {
            vertices,
            indices,
            texture_path: path,
            anim_frame_index: 0,
        }
    }

    pub fn anim_frame_index(&self) -> usize {
        self.anim_frame_index
    }

    pub fn set_anim_frame_index(&mut self, anim_frame_index: usize) -> usize {
        self.anim_frame_index = anim_frame_index % self.vertices.len();
        self.anim_frame_index
    }

    pub fn vertices(&self) -> &Vec<Vec<Vertex>> {
        &self.vertices
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }

    pub fn texture_path(&self) -> &PathBuf {
        &self.texture_path
    }
}
