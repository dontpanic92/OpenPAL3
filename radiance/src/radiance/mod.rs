pub mod core_engine;
pub use core_engine::CoreRadianceEngine;

use crate::{audio::OpenAlAudioEngine, rendering::VulkanRenderingEngine};
use std::error::Error;

pub fn create_radiance_engine(
    window: &crate::rendering::Window,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    let rendering_engine = Box::new(VulkanRenderingEngine::new(window)?);
    let audio_engine = Box::new(OpenAlAudioEngine::new());

    Ok(CoreRadianceEngine::new(rendering_engine, audio_engine))
}
