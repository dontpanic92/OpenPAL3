use std::cell::RefMut;

use super::VertexBuffer;

pub trait RenderObject: downcast_rs::Downcast {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>));

    /// Coarse local-space sort key for translucent draws — typically the
    /// midpoint of the source vertex AABB. Defaults to the origin so
    /// non-Vulkan backends and synthetic render objects keep working;
    /// each backend that participates in transparent sorting overrides
    /// this with the real centroid computed at construction time.
    fn local_centroid(&self) -> [f32; 3] {
        [0.0, 0.0, 0.0]
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
    /// PAL4 `UvAnimDriver` inventory dump can answer questions like
    /// "what texture does material `Material #6662332` actually use?"
    /// (water? foliage?). Default `None` for backends that don't track
    /// material textures distinctly.
    fn material_texture_name(&self) -> Option<&str> {
        None
    }
}

downcast_rs::impl_downcast!(RenderObject);
