pub mod vulkan;

pub trait Backend {
    fn new() -> Self;
}
