use crosscom::ComRc;

use super::{ComponentFactory, RenderTarget};
use crate::{comdef::IScene, imgui::ImguiFrame, scene::Viewport};
use std::rc::Rc;

pub trait RenderingEngine {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn render(&mut self, scene: ComRc<IScene>, viewport: Viewport, ui_frame: ImguiFrame);
    fn view_extent(&self) -> (u32, u32);
    fn component_factory(&self) -> Rc<dyn ComponentFactory>;

    /// Render `scene` into `target`'s offscreen color image. Distinct from
    /// `render`: no swapchain present, no imgui pass, viewport defaults to
    /// the target's full extent. The result is left in a layout the imgui
    /// renderer can sample, so the next swapchain pass can display it via
    /// `ui.image(target.imgui_texture_id(), ...)`.
    ///
    /// Should be called *before* `render` within the same frame; the
    /// backend is free to chain submissions via semaphores so the swapchain
    /// pass observes finished offscreen content.
    fn render_scene_to_target(
        &mut self,
        scene: ComRc<IScene>,
        target: &mut dyn RenderTarget,
    );
}
