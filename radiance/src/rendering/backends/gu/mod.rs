mod factory;

use alloc::rc::Rc;

use crate::rendering::{ComponentFactory, RenderingEngine};

use self::factory::GuComponentFactory;

pub struct GuRenderingEngine {}

impl GuRenderingEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderingEngine for GuRenderingEngine {
    fn render(&mut self, scene: &mut dyn crate::scene::Scene, ui_frame: crate::rendering::ui::ImguiFrame) {}

    fn view_extent(&self) -> (u32, u32) {
        (480, 320)
    }

    fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        Rc::new(GuComponentFactory::new())
    }
}
