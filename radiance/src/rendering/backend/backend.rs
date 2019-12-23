use crate::rendering::Vertex;

pub trait Backend {
    fn test(&mut self, vertices: &Vec<Vertex>) -> Result<(), Box<dyn std::error::Error>>;
}
