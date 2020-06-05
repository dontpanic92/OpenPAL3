use super::VertexBuffer;

pub trait RenderObject {
    fn update_vertices(&mut self, updater: &dyn Fn(&mut VertexBuffer));
}
