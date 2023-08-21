use std::cell::{RefCell, RefMut};

use crate::{
    math::Mat44,
    rendering::{Material, RenderObject, VertexBuffer},
};

use super::material::VitaGLMaterial;

pub struct VitaGLRenderObject {
    buffers: [u32; 4],
    vertex_buffer: Vec<f32>,
    tex_buffer: Vec<f32>,
    tex2_buffer: Vec<f32>,
    material: Box<VitaGLMaterial>,
    indices: Vec<u32>,

    model_matrix: RefCell<Mat44>,
}

impl RenderObject for VitaGLRenderObject {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>)) {}
}

impl VitaGLRenderObject {
    pub fn new(
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Box<dyn Material>,
        host_dynamic: bool,
    ) -> anyhow::Result<Self> {
        let mut buffers = [0; 4];
        let mut vertex_buffer = vec![];
        let mut tex_buffer = vec![];
        let mut tex2_buffer = vec![];
        for i in 0..vertices.count() {
            let p = vertices.position(i).unwrap();
            vertex_buffer.push(p.x);
            vertex_buffer.push(p.y);
            vertex_buffer.push(p.z);

            let t = vertices.tex_coord(i).unwrap();
            tex_buffer.push(t.x);
            tex_buffer.push(t.y);

            if let Some(t2) = vertices.tex_coord2(i) {
                tex2_buffer.push(t2.x);
                tex2_buffer.push(t2.y);
            } else {
                tex2_buffer.push(0.);
                tex2_buffer.push(0.);
            }
        }

        unsafe {
            use vitagl_sys::*;
            glGenBuffers(buffers.len() as i32, buffers.as_mut_ptr());
            glBindBuffer(GL_ARRAY_BUFFER, buffers[0]);
            glBufferData(
                GL_ARRAY_BUFFER,
                (std::mem::size_of::<f32>() * vertex_buffer.len()) as i32,
                vertex_buffer.as_ptr() as *const _,
                GL_STATIC_DRAW,
            );
            glBindBuffer(GL_ARRAY_BUFFER, 0);

            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, buffers[1]);
            glBufferData(
                GL_ELEMENT_ARRAY_BUFFER,
                (std::mem::size_of::<u16>() * indices.len()) as i32,
                indices.as_ptr() as *const _,
                GL_STATIC_DRAW,
            );
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);

            glBindBuffer(GL_ARRAY_BUFFER, buffers[2]);
            glBufferData(
                GL_ARRAY_BUFFER,
                (std::mem::size_of::<f32>() * tex_buffer.len()) as i32,
                tex_buffer.as_ptr() as *const _,
                GL_STATIC_DRAW,
            );
            glBindBuffer(GL_ARRAY_BUFFER, 0);

            if !tex2_buffer.is_empty() {
                glBindBuffer(GL_ARRAY_BUFFER, buffers[3]);
                glBufferData(
                    GL_ARRAY_BUFFER,
                    (std::mem::size_of::<f32>() * tex2_buffer.len()) as i32,
                    tex2_buffer.as_ptr() as *const _,
                    GL_STATIC_DRAW,
                );
                glBindBuffer(GL_ARRAY_BUFFER, 0);
            }
        }

        Ok(Self {
            buffers,
            vertex_buffer,
            tex_buffer,
            tex2_buffer,
            material: material.downcast::<VitaGLMaterial>().unwrap(),
            indices,
            model_matrix: RefCell::new(Mat44::new_identity()),
        })
    }

    pub fn buffers(&self) -> &[u32; 4] {
        &self.buffers
    }

    pub fn material(&self) -> &VitaGLMaterial {
        &self.material
    }

    pub fn vertex_buffer(&self) -> u32 {
        self.buffers[0]
    }

    pub fn index_buffer(&self) -> u32 {
        self.buffers[1]
    }

    pub fn tex_buffer(&self) -> u32 {
        self.buffers[2]
    }

    pub fn tex2_buffer(&self) -> u32 {
        self.buffers[3]
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

    // pub fn vertex_buffer(&self) -> &[f32] {
    //     &self.vertex_buffer
    // }

    // pub fn tex_buffer(&self) -> &[f32] {
    //     &self.tex_buffer
    // }

    // pub fn tex2_buffer(&self) -> &[f32] {
    //     &self.tex2_buffer
    // }

    // pub fn indices(&self) -> &[u32] {
    //     &self.indices
    // }
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
