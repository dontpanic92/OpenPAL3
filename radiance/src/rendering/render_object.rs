use super::VertexBuffer;

pub trait RenderObject {
    fn update_vertices(&mut self, updater: &mut dyn FnMut(&mut VertexBuffer));
}
