//! Backend-neutral offscreen color target.
//!
//! A `RenderTarget` owns a GPU color image that the rendering engine writes
//! to via [`RenderingEngine::render_scene_to_target`](super::RenderingEngine::render_scene_to_target)
//! and that the editor's imgui layer samples for in-window scene previews.
//!
//! The trait is intentionally narrow: only the values the editor / scripting
//! bridge need cross the abstraction boundary. The Vulkan implementation
//! holds substantial state behind it (color/depth images, framebuffer,
//! per-frame UBOs, command buffer, the registered imgui texture id).
//!
//! `imgui_texture_id` is returned as a raw `u64` to keep this trait
//! independent of the `imgui` crate. The Vulkan backend stores the
//! `imgui::TextureId` value here; the scripting bridge forwards it to
//! `ui.image(...)` without inspection.

pub trait RenderTarget {
    /// Current target extent in pixels.
    fn extent(&self) -> (u32, u32);

    /// Recreate the backing color/depth images at the new extent. No-op if
    /// the size already matches. After a successful resize the value
    /// returned by [`imgui_texture_id`](Self::imgui_texture_id) may change,
    /// so callers should re-fetch it every frame rather than caching across
    /// resizes.
    fn resize(&mut self, width: u32, height: u32);

    /// Opaque handle the imgui renderer can sample via `ui.image(id, ...)`.
    /// Encodes an `imgui::TextureId` on backends that use imgui-rs (Vulkan
    /// today).
    fn imgui_texture_id(&self) -> u64;

    /// Borrow as the backend-typed `VulkanRenderTarget`. Returns `None`
    /// on non-Vulkan backends. Lets `VulkanRenderingEngine::render_scene_to_target`
    /// recover the concrete type without going through `Any` /
    /// `downcast_rs` — mirrors the `RenderObjectHandle::as_vulkan` pattern
    /// on render objects.
    ///
    /// Default implementation returns `None`, so non-Vulkan backends and
    /// host test doubles don't need to override.
    #[cfg(vulkan)]
    fn as_vulkan_mut(&mut self) -> Option<&mut super::vulkan::VulkanRenderTarget> {
        None
    }
}
