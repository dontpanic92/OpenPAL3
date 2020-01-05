use crate::rendering;

pub trait RenderingBackend {
    fn new(window: &rendering::Window) -> Result<Self, Box<dyn std::error::Error>>
        where Self: std::marker::Sized;
    fn test(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}
