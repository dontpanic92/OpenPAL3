use super::Material;
use super::Vertex;

pub static TEXTURE_MISSING_TEXTURE_FILE: &'static [u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embed/textures/texture_missing.png"
));

pub struct RenderObject {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    material: Box<dyn Material>,
    pub is_dirty: bool,
}

impl RenderObject {
    pub fn new_with_data(
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        material: Box<dyn Material>,
    ) -> Self {
        Self {
            vertices,
            indices,
            material,
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

    pub fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }
}
