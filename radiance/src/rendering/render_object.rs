use super::Material;
use super::VertexBuffer;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct RenderObject {
    vertices: Arc<RwLock<VertexBuffer>>,
    indices: Arc<RwLock<Vec<u32>>>,
    material: Arc<RwLock<Box<dyn Material>>>,
    is_host_dynamic: bool,
    pub is_dirty: bool,
}

impl RenderObject {
    pub fn new_with_data(
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Box<dyn Material>,
    ) -> Self {
        Self {
            vertices: Arc::new(RwLock::new(vertices)),
            indices: Arc::new(RwLock::new(indices)),
            material: Arc::new(RwLock::new(material)),
            is_host_dynamic: false,
            is_dirty: false,
        }
    }

    pub fn new_host_dynamic_with_data(
        vertices: &Arc<RwLock<VertexBuffer>>,
        indices: &Arc<RwLock<Vec<u32>>>,
        material: &Arc<RwLock<Box<dyn Material>>>,
    ) -> Self {
        Self {
            vertices: vertices.clone(),
            indices: indices.clone(),
            material: material.clone(),
            is_host_dynamic: true,
            is_dirty: false,
        }
    }

    pub fn is_host_dynamic(&self) -> bool {
        self.is_host_dynamic
    }

    pub fn make_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn vertices(&self) -> RwLockReadGuard<VertexBuffer> {
        self.vertices.read().unwrap()
    }

    pub fn indices(&self) -> RwLockReadGuard<Vec<u32>> {
        self.indices.read().unwrap()
    }

    pub fn material(&self) -> RwLockReadGuard<Box<dyn Material>> {
        self.material.read().unwrap()
    }
}
