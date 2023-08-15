use crosscom::ComRc;

use super::ComponentFactory;
use crate::{comdef::IScene, imgui::ImguiFrame, scene::Viewport};
use std::rc::Rc;

pub trait RenderingEngine {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame);
    fn view_extent(&self) -> (u32, u32);
    fn component_factory(&self) -> Rc<dyn ComponentFactory>;
}
