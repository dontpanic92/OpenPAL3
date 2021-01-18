pub mod core_engine;
pub use core_engine::CoreRadianceEngine;

use crate::{
    application::Platform,
    audio::OpenAlAudioEngine,
    imgui::ImguiContext,
    input::WindowsInputEngine,
    rendering::{VulkanRenderingEngine, Window},
    scene::DefaultSceneManager,
};
use std::{cell::RefCell, error::Error, rc::Rc};

pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    let window = Window {
        hwnd: platform.hwnd(),
    };

    let imgui_context = Rc::new(RefCell::new(ImguiContext::new(platform)));
    let rendering_engine = Box::new(VulkanRenderingEngine::new(&window, imgui_context.clone())?);
    let audio_engine = Rc::new(OpenAlAudioEngine::new());
    let input_engine = WindowsInputEngine::new(platform);
    let scene_manager = Box::new(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        audio_engine,
        input_engine,
        imgui_context,
        scene_manager,
    ))
}
