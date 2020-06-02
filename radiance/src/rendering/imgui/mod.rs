pub mod vulkan;

use imgui::*;
use std::cell::RefCell;
use std::time::Instant;
use winapi::um::winuser;

pub struct ImguiContext {
    context: RefCell<Context>,
    last_frame: Instant,
}

impl ImguiContext {
    pub fn new(width: f32, height: f32) -> Self {
        let mut context = Context::create();
        let font_size = 32.;
        context.set_ini_filename(None);
        context.fonts().add_font(&[FontSource::TtfData {
            data: radiance_assets::FONT_SOURCE_HAN_SERIF,
            size_pixels: font_size,
            config: Some(FontConfig {
                rasterizer_multiply: 1.75,
                glyph_ranges: FontGlyphRanges::chinese_full(),
                ..FontConfig::default()
            }),
        }]);

        Self::init_platform(&mut context, width, height);
        Self {
            context: RefCell::new(context),
            last_frame: Instant::now(),
        }
    }

    pub fn draw_ui<F: FnOnce(&mut Ui)>(&mut self, draw: F) -> ImguiFrame {
        let io = self.context.get_mut().io_mut();
        self.last_frame = io.update_delta_time(self.last_frame);
        let mut ui = self.context.get_mut().frame();

        draw(&mut ui);
        std::mem::forget(ui);

        ImguiFrame { frame_begun: true }
    }

    fn init_platform(context: &mut Context, width: f32, height: f32) {
        context.set_platform_name(Some(ImString::from(format!(
            "radiance-imgui {}",
            env!("CARGO_PKG_VERSION"),
        ))));

        let io = context.io_mut();
        io.display_size = [width, height];
        io.backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS);
        io.backend_flags.insert(BackendFlags::HAS_SET_MOUSE_POS);
        io[Key::Tab] = winuser::VK_TAB as _;
        io[Key::LeftArrow] = winuser::VK_LEFT as _;
        io[Key::RightArrow] = winuser::VK_RIGHT as _;
        io[Key::UpArrow] = winuser::VK_UP as _;
        io[Key::DownArrow] = winuser::VK_DOWN as _;
        io[Key::Home] = winuser::VK_HOME as _;
        io[Key::End] = winuser::VK_END as _;
        io[Key::Insert] = winuser::VK_INSERT as _;
        io[Key::Delete] = winuser::VK_DELETE as _;
        io[Key::Backspace] = winuser::VK_BACK as _;
        io[Key::Space] = winuser::VK_SPACE as _;
        io[Key::Enter] = winuser::VK_RETURN as _;
        io[Key::Escape] = winuser::VK_ESCAPE as _;
        io[Key::A] = 'A' as _;
        io[Key::C] = 'C' as _;
        io[Key::V] = 'V' as _;
        io[Key::X] = 'X' as _;
        io[Key::Y] = 'Y' as _;
        io[Key::Z] = 'Z' as _;
    }
}

pub struct ImguiFrame {
    frame_begun: bool,
}

impl Default for ImguiFrame {
    fn default() -> Self {
        Self { frame_begun: false }
    }
}
