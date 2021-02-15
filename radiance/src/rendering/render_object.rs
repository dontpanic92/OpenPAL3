use super::VertexBuffer;

pub trait RenderObject: downcast_rs::Downcast {
    fn update_vertices(&mut self, updater: &mut dyn FnMut(&mut VertexBuffer));
}

downcast_rs::impl_downcast!(RenderObject);
