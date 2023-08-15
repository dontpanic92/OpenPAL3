use std::rc::Rc;

use crosscom::ComRc;
use imgui_rs_vitagl_renderer::ImguiRenderer;
use vitagl_sys::*;

use crate::{
    comdef::IScene,
    imgui::ImguiFrame,
    rendering::{ComponentFactory, RenderingEngine},
    scene::Viewport,
};

use super::factory::VitaGLComponentFactory;

pub struct VitaGLRenderingEngine {
    factory: Rc<VitaGLComponentFactory>,
    imgui: ImguiRenderer,
}

impl VitaGLRenderingEngine {
    pub fn new() -> Self {
        unsafe {
            vglInit(0x800000);
            vglWaitVblankStart(GL_TRUE as u8);
            glClearColor(0., 0., 0., 0.);

            glMatrixMode(GL_PROJECTION);
            glLoadIdentity();
            gluPerspective(90., 960. / 544., 0.01, 100.);

            glEnable(GL_DEPTH_TEST);
            glDepthFunc(GL_LESS);
        }

        Self {
            factory: Rc::new(VitaGLComponentFactory::new()),
            imgui: ImguiRenderer::new(),
        }
    }
}

impl RenderingEngine for VitaGLRenderingEngine {
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame) {
        let colors: [f32; 12] = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let vertices_front: [f32; 12] = [
            -0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, -0.5, 0.5, 0.5, -0.5,
        ];
        let vertices_back: [f32; 12] = [
            -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
        ];
        let vertices_left: [f32; 12] = [
            -0.5, -0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, -0.5, 0.5, 0.5,
        ];
        let vertices_right: [f32; 12] = [
            0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5,
        ];
        let vertices_top: [f32; 12] = [
            -0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
        ];
        let vertices_bottom: [f32; 12] = [
            -0.5, 0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
        ];

        let indices: [u16; 36] = [
            0, 1, 2, 1, 2, 3, 4, 5, 6, 5, 6, 7, 8, 9, 10, 9, 10, 11, 12, 13, 14, 13, 14, 15, 16,
            17, 18, 17, 18, 19, 20, 21, 22, 21, 22, 23,
        ];

        let mut color_array = [0.; 12 * 6];
        for i in 0..12 * 6 {
            color_array[i] = colors[i % 12];
        }

        let mut vertex_array = vec![];
        vertex_array.extend_from_slice(&vertices_front);
        vertex_array.extend_from_slice(&vertices_back);
        vertex_array.extend_from_slice(&vertices_left);
        vertex_array.extend_from_slice(&vertices_right);
        vertex_array.extend_from_slice(&vertices_top);
        vertex_array.extend_from_slice(&vertices_bottom);

        unsafe {
            glMatrixMode(GL_MODELVIEW);
            glLoadIdentity();
            glTranslatef(0., 0., -3.);

            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

            glEnableClientState(GL_VERTEX_ARRAY);
            glEnableClientState(GL_COLOR_ARRAY);
            glVertexPointer(
                3,
                GL_FLOAT,
                0,
                vertex_array.as_ptr() as *const std::ffi::c_void,
            );
            glColorPointer(
                3,
                GL_FLOAT,
                0,
                color_array.as_ptr() as *const std::ffi::c_void,
            );
            glRotatef(1., 0., 0., 1.);
            glRotatef(0.5, 0., 1., 0.);
            glDrawElements(
                GL_TRIANGLES,
                6 * 6,
                GL_UNSIGNED_SHORT,
                indices.as_ptr() as *const std::ffi::c_void,
            );
            glDisableClientState(GL_VERTEX_ARRAY);
            glDisableClientState(GL_COLOR_ARRAY);

            self.imgui.render();

            vglSwapBuffers(GL_FALSE as u8);
        }
    }

    fn view_extent(&self) -> (u32, u32) {
        (960, 544)
    }

    fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        self.factory.clone()
    }

    fn begin_frame(&mut self) {
        self.imgui.new_frame();
    }

    fn end_frame(&mut self) {}
}

impl Drop for VitaGLRenderingEngine {
    fn drop(&mut self) {
        unsafe {
            vglEnd();
        }
    }
}
