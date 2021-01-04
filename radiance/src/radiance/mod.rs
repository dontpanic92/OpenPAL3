pub mod core_engine;
pub use core_engine::CoreRadianceEngine;

use crate::{
    application::Platform,
    audio::OpenAlAudioEngine,
    input::WindowsInputEngine,
    rendering::{VulkanRenderingEngine, Window},
    scene::DefaultSceneManager,
};
use std::error::Error;

pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    let window = Window {
        hwnd: platform.hwnd(),
    };

    let rendering_engine = Box::new(VulkanRenderingEngine::new(&window)?);
    let audio_engine = Box::new(OpenAlAudioEngine::new());
    let input_engine = WindowsInputEngine::new(platform);
    let scene_manager = Box::new(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        audio_engine,
        input_engine,
        scene_manager,
    ))
}
