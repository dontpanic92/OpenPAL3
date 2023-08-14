use crosscom::ComRc;

use crate::{
    comdef::IScene,
    imgui::ImguiFrame,
    rendering::{ComponentFactory, RenderingEngine},
    scene::Viewport,
};

pub struct VitaGLRenderingEngine {}

impl VitaGLRenderingEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderingEngine for VitaGLRenderingEngine {
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame) {
        todo!()
    }

    fn view_extent(&self) -> (u32, u32) {
        (960, 544)
    }

    fn component_factory(&self) -> std::rc::Rc<dyn ComponentFactory> {
        todo!()
    }
}
