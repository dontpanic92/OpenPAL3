use super::Vertex;
use std::path::PathBuf;

pub static TEXTURE_MISSING_TEXTURE_FILE: &'static [u8] =
include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/embed/textures/texture_missing.png"));

pub struct RenderObject {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    texture_path: PathBuf,
    pub is_dirty: bool,
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
            is_dirty: false,
        }
    }

    pub fn new_with_data(vertices: Vec<Vertex>, indices: Vec<u32>, texture_path: &PathBuf) -> Self {
        Self {
            vertices,
            indices,
            texture_path: texture_path.clone(),
            is_dirty: false,
        }
    }

    pub fn update_vertices(&mut self, callback: &dyn Fn(&mut Vec<Vertex>)) {
        callback(&mut self.vertices);
        self.is_dirty = true;
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
