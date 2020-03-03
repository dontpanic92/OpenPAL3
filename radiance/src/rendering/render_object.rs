use super::Material;
use super::VertexBuffer;

pub struct RenderObject {
    vertices: VertexBuffer,
    indices: Vec<u32>,
    material: Box<dyn Material>,
    pub is_dirty: bool,
}

impl RenderObject {
    pub fn new_with_data(
        vertices: VertexBuffer,
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

    pub fn update_vertices(&mut self, callback: &dyn Fn(&mut VertexBuffer)) {
        callback(&mut self.vertices);
        self.is_dirty = true;
    }

    pub fn vertices(&self) -> &VertexBuffer {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }
}
