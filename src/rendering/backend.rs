pub mod vulkan;

pub trait Backend {
    fn test(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}
