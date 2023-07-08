#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod winit;

#[cfg_attr(target_os = "android", path = "clipboard_nop.rs")]
mod clipboard;

mod theme;

use self::theme::setup_theme;
#[cfg(not(target_os = "windows"))]
pub use self::winit::ImguiPlatform;
#[cfg(target_os = "windows")]
pub use windows::ImguiPlatform;

use crate::application::Platform;
use imgui::*;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

pub struct ImguiContext {
    context: Rc<RefCell<Context>>,
    platform: Rc<RefCell<ImguiPlatform>>,
}

impl ImguiContext {
    pub fn new(platform: &mut Platform) -> Self {
        let mut context = Context::create();
        context.set_ini_filename(None);
        setup_theme(&mut context);

        context.style_mut().scale_all_sizes(platform.dpi_scale());
        context.fonts().add_font(&[FontSource::TtfData {
            data: radiance_assets::FONT_SOURCE_HAN_SERIF,
            size_pixels: 28. * platform.dpi_scale(),
            config: Some(FontConfig {
                rasterizer_multiply: 1.75,
                glyph_ranges: FontGlyphRanges::chinese_full(),
                ..FontConfig::default()
            }),
        }]);

        context.fonts().add_font(&[FontSource::TtfData {
            data: radiance_assets::FONT_SOURCE_HAN_SERIF,
            size_pixels: 18. * platform.dpi_scale(),
            config: Some(FontConfig {
                rasterizer_multiply: 1.75,
                glyph_ranges: FontGlyphRanges::chinese_full(),
                ..FontConfig::default()
            }),
        }]);

        context.io_mut().config_flags |= imgui::ConfigFlags::DOCKING_ENABLE;

        if let Some(backend) = clipboard::init() {
            context.set_clipboard_backend(backend);
        } else {
            log::error!("Failed to initialize clipboard support");
        }

        let context = Rc::new(RefCell::new(context));
        let platform = ImguiPlatform::new(context.clone(), platform);
        Self { context, platform }
    }

    pub fn draw_ui<F: FnOnce(&Ui)>(&mut self, delta_sec: f32, draw: F) -> ImguiFrame {
        self.platform.borrow_mut().new_frame(delta_sec);

        let mut context = self.context.borrow_mut();
        let ui = context.frame();
        draw(&ui);

        ImguiFrame { frame_begun: true }
    }

    pub fn context_mut(&mut self) -> RefMut<Context> {
        self.context.borrow_mut()
    }
}

pub struct ImguiFrame {
    pub frame_begun: bool,
}

impl Default for ImguiFrame {
    fn default() -> Self {
        Self { frame_begun: false }
    }
}

impl Drop for ImguiFrame {
    fn drop(&mut self) {
        if self.frame_begun {
            unsafe {
                sys::igEndFrame();
            }
        }
    }
}
