use crosscom::ComRc;

use super::{ComponentFactory, RenderTarget};
use crate::{comdef::IScene, imgui::ImguiFrame, scene::Viewport};
use std::rc::Rc;

/// Raw CPU-side copy of the most recently presented framebuffer.
/// Always packed as 8-bit RGBA in linear row-major order with no
/// padding between rows. The agent server's HTTP transport encodes
/// this as PNG on its own thread so the game thread isn't blocked on
/// compression.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, RGBA8.
    pub rgba: Vec<u8>,
}

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
    fn render_scene_to_target(&mut self, scene: ComRc<IScene>, target: &mut dyn RenderTarget);

    /// Read back the most recently *presented* swapchain image into a
    /// CPU-mapped RGBA buffer. Returns `None` when no frame has been
    /// presented yet, the backend lacks a presentable surface
    /// (headless / pre-init), or the swapchain format is not a
    /// supported 8-bit color format.
    ///
    /// Cost: the default Vulkan implementation submits a one-shot
    /// command buffer and `vkDeviceWaitIdle`s. Intended for occasional
    /// one-off captures (debug screenshots, agent-server requests),
    /// not per-frame use.
    fn capture_last_frame(&mut self) -> Option<CapturedFrame> {
        None
    }
}
