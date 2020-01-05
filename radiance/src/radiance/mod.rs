pub mod core_engine;

pub use core_engine::CoreRadianceEngine;
use std::error::Error;
use crate::rendering;
use crate::rendering::backend;

pub trait RadianceEngine {
    fn update(&mut self);
}

pub type DefaultRadianceEngine<TBackend> = CoreRadianceEngine<rendering::CoreRenderingEngine<TBackend>>;

pub fn create_default_radiance_engine<TBackend: backend::RenderingBackend>(window: &crate::rendering::Window) -> Result<DefaultRadianceEngine<TBackend>, Box<dyn Error>> {
    let rendering_engine = rendering::create_core_rendering_engine(window)?;
    Ok(DefaultRadianceEngine::new(rendering_engine))
}

