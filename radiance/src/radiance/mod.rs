mod core_engine;
mod debugging;

pub use core_engine::CoreRadianceEngine;
pub use debugging::DebugLayer;

use crate::{
    application::Platform, imgui::ImguiContext, input::GenericInputEngine,
    scene::DefaultSceneManager,
};
use std::{cell::RefCell, error::Error, rc::Rc};

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    use crate::{audio::OpenAlAudioEngine, rendering::VulkanRenderingEngine};

    let imgui_context = Rc::new(RefCell::new(ImguiContext::new(platform)));
    #[cfg(target_os = "windows")]
    let window = &crate::rendering::Window {
        hwnd: platform.hwnd(),
    };
    #[cfg(not(target_os = "windows"))]
    let window = platform.get_window();

    let rendering_engine = Rc::new(RefCell::new(VulkanRenderingEngine::new(
        window,
        imgui_context.clone(),
    )?));

    #[cfg(target_os = "android")]
    {
        use winit::event::Event;
        let rendering_engine_clone = rendering_engine.clone();
        platform.add_message_callback(Box::new(move |window, event| {
            let mut rendering_engine = rendering_engine_clone.borrow_mut();
            match event {
                Event::Suspended => {
                    rendering_engine.drop_surface();
                }
                Event::Resumed => {
                    rendering_engine.recreate_surface(window).unwrap();
                }
                _ => (),
            }
        }));
    }
    let audio_engine = Rc::new(OpenAlAudioEngine::new());
    let input_engine = GenericInputEngine::new(platform);
    let scene_manager = Box::new(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        audio_engine,
        input_engine,
        imgui_context,
        scene_manager,
    ))
}

#[cfg(target_os = "psp")]
pub fn create_radiance_engine(platform: &mut Platform) {
    use crate::{audio::NullAudioEngine, rendering::GuRenderingEngine};

    let imgui_context = Rc::new(RefCell::new(ImguiContext::new(platform)));
    let rendering_engine = Rc::new(RefCell::new(GuRenderingEngine::new()?));
    let audio_engine = Rc::new(NullAudioEngine::new());
    let input_engine = GenericInputEngine::new(platform);
    let scene_manager = Box::new(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        audio_engine,
        input_engine,
        imgui_context,
        scene_manager,
    ))
}
