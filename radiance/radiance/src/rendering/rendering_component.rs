use super::RenderObjectHandle;

/// Per-entity collection of render objects, owned by an `IEntity` and
/// consumed by the active rendering engine each frame.
///
/// Holds two parallel views of the same underlying objects:
///
/// - `objects` — every handle, regardless of backend. Used by polymorphic
///   trait-method consumers (`UvAnimDriver`, the morph animator, the
///   skinned-mesh updater) through [`Self::render_objects`].
/// - `vulkan_objects` — backend-typed `Rc<VulkanRenderObject>` clones
///   populated by the Vulkan factory at insertion time, so the rendering
///   engine's per-frame draw loop iterates concrete types without ever
///   calling `downcast_ref`. Mirrors how `VulkanMaterial` already holds
///   `Rc<VulkanShader>` and `Rc<VulkanTexture>` directly. Empty on
///   non-Vulkan backends and on the cfg path where vulkan isn't compiled.
pub struct RenderingComponent {
    objects: Vec<RenderObjectHandle>,
    #[cfg(vulkan)]
    vulkan_objects: Vec<std::rc::Rc<super::vulkan::VulkanRenderObject>>,
    #[cfg(vitagl)]
    vitagl_objects: Vec<std::rc::Rc<super::vitagl::VitaGLRenderObject>>,
}

impl RenderingComponent {
    pub fn new() -> Self {
        RenderingComponent {
            objects: vec![],
            #[cfg(vulkan)]
            vulkan_objects: vec![],
            #[cfg(vitagl)]
            vitagl_objects: vec![],
        }
    }

    pub fn push_render_object(&mut self, handle: RenderObjectHandle) {
        #[cfg(vulkan)]
        if let Some(v) = handle.as_vulkan() {
            self.vulkan_objects.push(v.clone());
        }
        #[cfg(vitagl)]
        if let Some(v) = handle.as_vitagl() {
            self.vitagl_objects.push(v.clone());
        }
        self.objects.push(handle);
    }

    pub fn render_objects(&self) -> &[RenderObjectHandle] {
        &self.objects
    }

    /// Backend-typed view of the same render objects, populated for
    /// handles that originated from the Vulkan factory. The slice
    /// length matches `render_objects()` exactly when every render
    /// object in this component came from the Vulkan factory, which is
    /// the only configuration that uses this accessor today.
    #[cfg(vulkan)]
    pub fn vulkan_render_objects(&self) -> &[std::rc::Rc<super::vulkan::VulkanRenderObject>] {
        &self.vulkan_objects
    }

    /// VitaGL counterpart to [`Self::vulkan_render_objects`].
    #[cfg(vitagl)]
    pub fn vitagl_render_objects(&self) -> &[std::rc::Rc<super::vitagl::VitaGLRenderObject>] {
        &self.vitagl_objects
    }
}
