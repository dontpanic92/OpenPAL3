mod core_engine;
mod task_manager;
mod ui_frame;
mod ui_layer;
mod ui_manager;

pub use core_engine::CoreRadianceEngine;
pub use task_manager::{TaskHandle, TaskManager};
pub use ui_frame::UiFrameRenderer;
pub use ui_layer::{UiLayerBand, UiLayerHandle, UiLayerStack};
pub use ui_manager::UiManager;

use crosscom::ComRc;

use crate::{application::Platform, audio::OpenAlAudioEngine, scene::DefaultSceneManager};
use std::{cell::RefCell, error::Error, rc::Rc};

pub fn create_radiance_engine(
    platform: &mut Platform,
    options: crate::rendering::RenderingEngineOptions,
) -> Result<CoreRadianceEngine, Box<dyn Error>> {
    let ui_manager = Rc::new(UiManager::new(platform));

    #[cfg(windows)]
    let window = &crate::rendering::Window {
        hwnd: platform.hwnd(),
    };

    #[cfg(any(linux, macos, android))]
    let window = platform.get_window();

    // If the caller asked for Logical mode but did not supply an
    // explicit extent, derive one from the live window. This keeps
    // every host application from having to compute the same
    // (physical / dpi_scale) value themselves.
    #[allow(unused_mut)]
    let mut options = options;
    if matches!(
        options.scene_scale_mode,
        crate::rendering::SceneScaleMode::Logical
    ) && options.logical_extent.is_none()
    {
        options.logical_extent = platform.logical_inner_extent();
    }

    #[cfg(vulkan)]
    let rendering_engine = Rc::new(RefCell::new(crate::rendering::VulkanRenderingEngine::new(
        &window,
        &ui_manager.imgui_context(),
        options,
    )?));

    #[cfg(vitagl)]
    let rendering_engine = Rc::new(RefCell::new(crate::rendering::VitaGLRenderingEngine::new()));

    #[cfg(vitagl)]
    let _ = options;

    #[cfg(target_os = "android")]
    {
        use crate::application::winit::LifecycleEvent;
        let rendering_engine_clone = rendering_engine.clone();
        let w = window.clone();
        platform.add_lifecycle_callback(Box::new(move |event| {
            let mut rendering_engine = rendering_engine_clone.borrow_mut();
            match event {
                LifecycleEvent::Suspended => {
                    rendering_engine.drop_surface();
                }
                LifecycleEvent::Resumed => {
                    rendering_engine.recreate_surface(&w).unwrap();
                }
            }
        }));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use crate::rendering::RenderingEngine;
        use winit::event::WindowEvent;
        let rendering_engine_clone = rendering_engine.clone();
        let window_cb = window.clone();
        platform.add_window_event_callback(Box::new(move |_window_id, event| {
            // On resize / DPI change, re-track the scene's logical render
            // extent to the window's new logical (DPI-independent) size and
            // rebuild the swapchain. Without this, Logical mode keeps
            // upscaling the fixed boot-time offscreen and stretches the
            // scene when the window's aspect ratio changes.
            match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    let physical = window_cb.inner_size();
                    let scale = (window_cb.scale_factor() as f64).max(0.0001);
                    let lw = (((physical.width as f64) / scale).round() as u32).max(1);
                    let lh = (((physical.height as f64) / scale).round() as u32).max(1);
                    rendering_engine_clone.borrow_mut().notify_resized((lw, lh));
                }
                _ => {}
            }
        }));
    }

    let audio_engine = Rc::new(OpenAlAudioEngine::new());
    let input_engine = crate::input::CoreInputEngine::new(platform);
    let scene_manager = ComRc::from_object(DefaultSceneManager::new());

    Ok(CoreRadianceEngine::new(
        rendering_engine,
        audio_engine,
        input_engine,
        ui_manager,
        scene_manager,
    ))
}
