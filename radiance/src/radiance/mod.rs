pub mod core_engine;

use crate::rendering;
pub use core_engine::CoreRadianceEngine;
use std::error::Error;

pub trait RadianceEngine {
    fn load_scene(&mut self);
    fn unload_scene(&mut self);
    fn update(&mut self);
}

pub fn create_radiance_engine<TRenderingEngine: rendering::RenderingEngine>(
    window: &crate::rendering::Window,
) -> Result<CoreRadianceEngine<TRenderingEngine>, Box<dyn Error>> {
    let rendering_engine = TRenderingEngine::new(window)?;
    Ok(CoreRadianceEngine::<TRenderingEngine>::new(
        rendering_engine,
    ))
}
