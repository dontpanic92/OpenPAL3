use std::cell::RefMut;

use crate::rendering::{RenderObject, VertexBuffer};

pub struct VitaGLRenderObject {}

impl RenderObject for VitaGLRenderObject {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>)) {}
}

impl VitaGLRenderObject {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
