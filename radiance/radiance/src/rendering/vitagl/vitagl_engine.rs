use std::rc::Rc;

use crosscom::ComRc;
use imgui_rs_vitagl_renderer::ImguiRenderer;
use vitagl_sys::*;

use crate::{
    comdef::IScene,
    imgui::ImguiFrame,
    math::Mat44,
    rendering::{ComponentFactory, RenderingComponent, RenderingEngine},
    scene::Viewport,
};

use super::{factory::VitaGLComponentFactory, render_object::VitaGLRenderObject};

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
        }

        Self {
            factory: Rc::new(VitaGLComponentFactory::new()),
            imgui: ImguiRenderer::new(),
        }
    }
}

impl RenderingEngine for VitaGLRenderingEngine {
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame) {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
            glEnable(GL_DEPTH_TEST);
            glDepthFunc(GL_LESS);
        }

        let (view, proj) = {
            let c = scene.camera();
            let camera = c.borrow();
            let view = Mat44::inversed(camera.transform().matrix());
            let proj = camera.projection_matrix();
            (view, *proj)
        };

        let rc: Vec<_> = scene
            .visible_entities()
            .iter()
            .filter_map(|e| {
                e.get_rendering_component()
                    .and_then(|c| Some((c, e.world_transform().matrix().clone())))
            })
            .collect();
        let r_objects: Vec<&VitaGLRenderObject> = rc
            .iter()
            .map(|(c, m)| {
                let m = m.clone();
                c.render_objects().iter().map(move |o| (o, m))
            })
            .flatten()
            .filter_map(|(c, m)| {
                c.downcast_ref::<VitaGLRenderObject>()
                    .and_then(|c| Some((c, m)))
            })
            .map(|(c, m)| {
                c.set_model_matrix(m);
                c
            })
            .collect();

        let mut objects_by_material = vec![];
        for obj in r_objects {
            objects_by_material.push((obj.material(), vec![obj]));
        }
        for (material, objects) in objects_by_material {
            unsafe {
                glUseProgram(material.shader().program());
                glUniformMatrix4fv(
                    material.shader().uniform_view_matrix(),
                    1,
                    GL_FALSE as u8,
                    view.floats().as_ptr() as *const _,
                );
                glUniformMatrix4fv(
                    material.shader().uniform_projection_matrix(),
                    1,
                    GL_FALSE as u8,
                    proj.floats().as_ptr() as *const _,
                );

                let textures = material.textures();
                if textures.len() > 0 {
                    glActiveTexture(GL_TEXTURE0);
                    glBindTexture(GL_TEXTURE_2D, textures[0].texture_id());
                }

                if textures.len() > 1 {
                    glActiveTexture(GL_TEXTURE1);
                    glBindTexture(GL_TEXTURE_2D, textures[1].texture_id());
                }

                for obj in objects {
                    glUniformMatrix4fv(
                        material.shader().uniform_model_matrix(),
                        1,
                        GL_FALSE as u8,
                        obj.model_matrix().floats().as_ptr() as *const _,
                    );

                    glEnableVertexAttribArray(0);
                    glBindBuffer(GL_ARRAY_BUFFER, obj.vertex_buffer());
                    glVertexAttribPointer(
                        0,
                        3,
                        GL_FLOAT,
                        GL_FALSE as u8,
                        obj.stride(),
                        obj.vertex_offset() as *const _,
                    );

                    glEnableVertexAttribArray(1);
                    glBindBuffer(GL_ARRAY_BUFFER, obj.vertex_buffer());
                    glVertexAttribPointer(
                        1,
                        2,
                        GL_FLOAT,
                        GL_FALSE as u8,
                        obj.stride(),
                        obj.tex_coord_offset() as *const _,
                    );

                    if textures.len() > 1 {
                        glEnableVertexAttribArray(2);
                        glBindBuffer(GL_ARRAY_BUFFER, obj.vertex_buffer());
                        glVertexAttribPointer(
                            2,
                            2,
                            GL_FLOAT,
                            GL_FALSE as u8,
                            obj.stride(),
                            obj.tex_coord2_offset() as *const _,
                        );
                    }

                    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, obj.index_buffer());
                    glDrawElements(
                        GL_TRIANGLES,
                        obj.index_count(),
                        GL_UNSIGNED_INT,
                        std::ptr::null(),
                    );
                }
            }
        }

        unsafe {
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
    fn drop(&mut self) {}
}
