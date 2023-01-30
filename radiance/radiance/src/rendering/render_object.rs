use std::cell::RefMut;

use super::VertexBuffer;

pub trait RenderObject: downcast_rs::Downcast {
    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>));
}

downcast_rs::impl_downcast!(RenderObject);
