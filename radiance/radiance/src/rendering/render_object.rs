use std::cell::RefMut;
use std::rc::Rc;

use super::VertexBuffer;

pub trait RenderObject {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>));

    /// Coarse local-space sort key for translucent draws — typically the
    /// midpoint of the source vertex AABB. Defaults to the origin so
    /// non-Vulkan backends and synthetic render objects keep working;
    /// each backend that participates in transparent sorting overrides
    /// this with the real centroid computed at construction time.
    fn local_centroid(&self) -> [f32; 3] {
        [0.0, 0.0, 0.0]
    }

    /// Local-space axis-aligned bounding box `(min, max)` of the
    /// render object's vertex positions, or `None` if the backend
    /// doesn't track it. Used by the engine-side frustum culler to
    /// reject offscreen render objects before bucketing.
    ///
    /// Returning `None` is the conservative "always visible" choice
    /// — backends that don't compute bounds (or render objects built
    /// from synthetic / dynamic geometry where bounds would lie) keep
    /// being drawn unconditionally, matching pre-culling behaviour.
    fn local_aabb(&self) -> Option<([f32; 3], [f32; 3])> {
        None
    }

    /// Re-upload the underlying material's UV affine
    /// (`scale = (sx, sy)`, `offset = (tx, ty)`) so the vertex shader's
    /// `uv = inTexCoord * scale + offset` produces the requested
    /// transform on the next draw.
    ///
    /// Default implementation is a no-op so backends that don't yet
    /// support per-material UV animation (vitagl, dummy) silently ignore
    /// it; the runtime PAL4 `UvAnimComponent` calls this unconditionally
    /// against every render object it knows about.
    fn set_uv_xform(&self, _scale: [f32; 2], _offset: [f32; 2]) {}

    /// Returns the source material's `debug_name`, when available.
    ///
    /// PAL4's water-UV-animation driver uses this to find render objects
    /// whose material was stamped (in the DFF loader) with the
    /// `PLUGIN_USERDATA name` link key — only those should receive the
    /// per-frame UV transform. Backends that don't track this return
    /// `None` (default), which keeps the animation disabled for them
    /// rather than leaking onto every render object.
    fn material_debug_name(&self) -> Option<&str> {
        None
    }

    /// Returns the underlying material's first texture name, when
    /// available. Diagnostic-only — exposed so debug overlays and the
    /// PAL4 `UvAnimationComponent` inventory dump can answer questions like
    /// "what texture does material `Material #6662332` actually use?"
    /// (water? foliage?). Default `None` for backends that don't track
    /// material textures distinctly.
    fn material_texture_name(&self) -> Option<&str> {
        None
    }
}

/// Owning handle to a render object that pairs the cross-backend trait
/// object view with an optional backend-typed `Rc` to the same underlying
/// instance.
///
/// The factory constructs one of these per render object at *insertion*
/// time, so the rendering engine can iterate backend-typed handles in the
/// per-frame draw loop without ever calling `downcast_ref`. Mirrors how
/// `VulkanMaterial` already holds `Rc<VulkanShader>` / `Vec<Rc<VulkanTexture>>`
/// directly rather than going through `dyn Shader` / `dyn Texture`.
///
/// The `dyn_view` field stays in sync with the backend-typed slot — both
/// reference the same heap allocation — so polymorphic callers like
/// `UvAnimationComponent` keep working unchanged through [`Self::as_dyn`].
pub struct RenderObjectHandle {
    dyn_view: Rc<dyn RenderObject>,
    #[cfg(vulkan)]
    vulkan: Option<Rc<super::vulkan::VulkanRenderObject>>,
    #[cfg(vitagl)]
    vitagl: Option<Rc<super::vitagl::VitaGLRenderObject>>,
}

impl RenderObjectHandle {
    /// Construct a handle from a backend-erased `Rc<dyn RenderObject>`.
    /// Backends that don't have a typed-handle pathway (test doubles)
    /// use this; the engine then has no typed view available for these
    /// handles.
    pub fn from_dyn(obj: Rc<dyn RenderObject>) -> Self {
        Self {
            dyn_view: obj,
            #[cfg(vulkan)]
            vulkan: None,
            #[cfg(vitagl)]
            vitagl: None,
        }
    }

    /// Construct a handle from a typed `Rc<VulkanRenderObject>`. The
    /// trait-object view shares the same allocation, so polymorphic
    /// readers and the engine's typed iteration both see the same
    /// underlying object.
    #[cfg(vulkan)]
    pub fn from_vulkan(obj: Rc<super::vulkan::VulkanRenderObject>) -> Self {
        Self {
            dyn_view: obj.clone(),
            vulkan: Some(obj),
            #[cfg(vitagl)]
            vitagl: None,
        }
    }

    /// Construct a handle from a typed `Rc<VitaGLRenderObject>`. See
    /// [`Self::from_vulkan`] for the design rationale; this is the
    /// symmetric VitaGL constructor.
    #[cfg(vitagl)]
    pub fn from_vitagl(obj: Rc<super::vitagl::VitaGLRenderObject>) -> Self {
        Self {
            dyn_view: obj.clone(),
            #[cfg(vulkan)]
            vulkan: None,
            vitagl: Some(obj),
        }
    }

    /// Borrow as the cross-backend trait surface. Used by callers that
    /// only need the neutral trait methods (`update_vertices`,
    /// `material_debug_name`, `set_uv_xform`, …).
    pub fn as_dyn(&self) -> &dyn RenderObject {
        &*self.dyn_view
    }

    /// Borrow the backend-typed handle, when present. The Vulkan
    /// rendering engine uses this in its per-frame loop instead of
    /// downcasting through `Any`.
    #[cfg(vulkan)]
    pub fn as_vulkan(&self) -> Option<&Rc<super::vulkan::VulkanRenderObject>> {
        self.vulkan.as_ref()
    }

    /// VitaGL counterpart to [`Self::as_vulkan`].
    #[cfg(vitagl)]
    pub fn as_vitagl(&self) -> Option<&Rc<super::vitagl::VitaGLRenderObject>> {
        self.vitagl.as_ref()
    }
}
