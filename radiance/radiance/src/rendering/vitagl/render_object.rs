use std::cell::{RefCell, RefMut};

use crate::{
    math::Mat44,
    rendering::{Material, RenderObject, VertexBuffer, VertexComponents},
};

use super::material::VitaGLMaterial;

pub struct VitaGLRenderObject {
    buffers: [u32; 2],
    vertices: RefCell<VertexBuffer>,
    material: Box<VitaGLMaterial>,
    indices: Vec<u32>,

    model_matrix: RefCell<Mat44>,
}

impl RenderObject for VitaGLRenderObject {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>)) {
        updater(self.vertices.borrow_mut());
        unsafe {
            use vitagl_sys::*;
            glBindBuffer(GL_ARRAY_BUFFER, self.buffers[0]);
            glBufferData(
                GL_ARRAY_BUFFER,
                self.vertices.borrow().data().len() as i32,
                self.vertices.borrow().data().as_ptr() as *const _,
                GL_DYNAMIC_DRAW,
            );
        }
    }
}

impl VitaGLRenderObject {
    pub fn new(
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Box<dyn Material>,
        host_dynamic: bool,
    ) -> anyhow::Result<Self> {
        let mut buffers = [0; 2];

        unsafe {
            use vitagl_sys::*;
            glGenBuffers(buffers.len() as i32, buffers.as_mut_ptr());
            glBindBuffer(GL_ARRAY_BUFFER, buffers[0]);
            glBufferData(
                GL_ARRAY_BUFFER,
                vertices.data().len() as i32,
                vertices.data().as_ptr() as *const _,
                if host_dynamic {
                    GL_DYNAMIC_DRAW
                } else {
                    GL_STATIC_DRAW
                },
            );
            glBindBuffer(GL_ARRAY_BUFFER, 0);

            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, buffers[1]);
            glBufferData(
                GL_ELEMENT_ARRAY_BUFFER,
                (std::mem::size_of::<u32>() * indices.len()) as i32,
                indices.as_ptr() as *const _,
                GL_STATIC_DRAW,
            );
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);
        }

        Ok(Self {
            buffers,
            vertices: RefCell::new(vertices),
            material: material.downcast::<VitaGLMaterial>().unwrap(),
            indices,
            model_matrix: RefCell::new(Mat44::new_identity()),
        })
    }

    pub fn buffers(&self) -> &[u32; 2] {
        &self.buffers
    }

    pub fn material(&self) -> &VitaGLMaterial {
        &self.material
    }

    pub fn stride(&self) -> i32 {
        self.vertices.borrow().layout().size() as i32
    }

    pub fn vertex_offset(&self) -> usize {
        self.vertices
            .borrow()
            .layout()
            .get_offset(VertexComponents::POSITION)
            .unwrap() as usize
    }

    pub fn normal_offset(&self) -> usize {
        self.vertices
            .borrow()
            .layout()
            .get_offset(VertexComponents::NORMAL)
            .unwrap() as usize
    }

    pub fn tex_coord_offset(&self) -> usize {
        self.vertices
            .borrow()
            .layout()
            .get_offset(VertexComponents::TEXCOORD)
            .unwrap() as usize
    }

    pub fn tex_coord2_offset(&self) -> usize {
        self.vertices
            .borrow()
            .layout()
            .get_offset(VertexComponents::TEXCOORD2)
            .unwrap() as usize
    }

    pub fn vertex_buffer(&self) -> u32 {
        self.buffers[0]
    }

    pub fn index_buffer(&self) -> u32 {
        self.buffers[1]
    }

    pub fn index_count(&self) -> i32 {
        self.indices.len() as i32
    }

    pub fn set_model_matrix(&self, model_matrix: Mat44) {
        self.model_matrix.replace(model_matrix);
    }

    pub fn model_matrix(&self) -> Mat44 {
        self.model_matrix.borrow().clone()
    }
}

impl Drop for VitaGLRenderObject {
    fn drop(&mut self) {
        unsafe {
            vitagl_sys::glDeleteBuffers(
                self.buffers.len() as i32,
                self.buffers.as_ptr() as *const _,
            );
        }
    }
}
