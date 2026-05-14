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
}

downcast_rs::impl_downcast!(RenderObject);
