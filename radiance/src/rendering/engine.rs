use super::Window;
use crate::scene::Scene;

pub trait RenderingEngine {
    fn new(window: &Window) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: std::marker::Sized;

    fn render(&mut self, scene: &mut dyn Scene);
    fn scene_loaded(&mut self, scene: &mut dyn Scene);

    fn view_extent(&self) -> (u32, u32);
}
