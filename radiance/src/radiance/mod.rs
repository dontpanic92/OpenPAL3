mod core_engine;
mod debugging;

pub use core_engine::CoreRadianceEngine;
pub use debugging::DebugLayer;

use crate::{
    application::Platform, input::GenericInputEngine, scene::DefaultSceneManager, ui::ImguiContext,
};

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, Box<dyn std::error::Error>> {
    use crate::{audio::OpenAlAudioEngine, rendering::VulkanRenderingEngine};
    use std::{cell::RefCell, rc::Rc};

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
pub fn create_radiance_engine(
    platform: &mut Platform,
) -> Result<CoreRadianceEngine, alloc::boxed::Box<dyn core2::error::Error>> {
    use crate::{audio::NullAudioEngine, rendering::backends::gu::GuRenderingEngine};
    use alloc::rc::Rc;
    use core::cell::RefCell;

    let imgui_context = Rc::new(RefCell::new(ImguiContext::new(platform)));
    let rendering_engine = Rc::new(RefCell::new(GuRenderingEngine::new()));
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
