mod core_engine;
mod debugging;
pub use core_engine::CoreRadianceEngine;
pub use debugging::DebugLayer;

use crate::{
    application::Platform, imgui::ImguiContext, input::GenericInputEngine,
    media::FFMpegMediaEngine, rendering::VulkanRenderingEngine, scene::DefaultSceneManager,
};
use std::{cell::RefCell, error::Error, rc::Rc};

pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    let imgui_context = Rc::new(RefCell::new(ImguiContext::new(platform)));
    #[cfg(target_os = "windows")]
    let window = &crate::rendering::Window {
        hwnd: platform.hwnd(),
    };
    #[cfg(not(target_os = "windows"))]
    let window = platform.get_window();
    let rendering_engine = Box::new(VulkanRenderingEngine::new(window, imgui_context.clone())?);
    let media_engine = Rc::new(FFMpegMediaEngine::new());
    let input_engine = GenericInputEngine::new(platform);
    let scene_manager = Box::new(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        media_engine,
        input_engine,
        imgui_context,
        scene_manager,
    ))
}
