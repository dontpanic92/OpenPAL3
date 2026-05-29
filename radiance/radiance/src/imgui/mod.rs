#[cfg_attr(any(android, vita), path = "clipboard_nop.rs")]
mod clipboard;

mod platform;
mod theme;

pub use self::theme::{available_themes, menu_item_padding, DEFAULT_THEME};

use self::theme::{
    apply_theme as apply_named_theme, resolve_theme_name, scale_menu_item_padding,
};

use crate::application::Platform;
use imgui::*;
use platform::ImguiPlatform;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

pub struct FontIndex;
impl FontIndex {
    pub const LARGE_FONT: usize = 0;

    #[cfg(not(vita))]
    pub const SMALL_FONT: usize = 1;

    #[cfg(vita)]
    pub const SMALL_FONT: usize = 0;
}

pub struct ImguiContext {
    context: Rc<RefCell<Context>>,
    platform: Rc<RefCell<ImguiPlatform>>,
    dpi_scale: f32,
    current_theme: RefCell<String>,
    pending_theme: RefCell<Option<String>>,
}

impl ImguiContext {
    pub fn new(platform: &mut Platform) -> Self {
        let mut context = Context::create();
        context.set_ini_filename(None);
        let applied = apply_named_theme(&mut context, DEFAULT_THEME);

        let dpi_scale = platform.dpi_scale();
        context.style_mut().scale_all_sizes(dpi_scale);
        scale_menu_item_padding(dpi_scale);
        #[cfg(not(vita))]
        {
            add_font_with_lucide(&mut context, 28. * platform.dpi_scale());
            add_font_with_lucide(&mut context, 18. * platform.dpi_scale());
        }

        #[cfg(vita)]
        context.fonts().add_font(&[FontSource::TtfData {
            data: radiance_assets::FONT_LXGW_NEOXIHEI_SCREEN,
            size_pixels: 18. * platform.dpi_scale(),
            config: Some(FontConfig {
                rasterizer_multiply: 1.,
                glyph_ranges: FontGlyphRanges::chinese_simplified_common(),
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
        Self {
            context,
            platform,
            dpi_scale,
            current_theme: RefCell::new(applied.to_string()),
            pending_theme: RefCell::new(None),
        }
    }

    /// Request a theme switch. The actual style mutation is deferred to
    /// the next `draw_ui` so callers reached from inside the current
    /// imgui frame (e.g. a script-driven menu item) don't re-enter the
    /// `Context` borrow that `draw_ui` holds. Returns the name that
    /// will be applied (falls back to [`DEFAULT_THEME`] on unknown
    /// names).
    pub fn apply_theme(&self, name: &str) -> &'static str {
        let resolved = resolve_theme_name(name);
        *self.pending_theme.borrow_mut() = Some(resolved.to_string());
        resolved
    }

    fn drain_pending_theme(&self, context: &mut Context) {
        let pending = self.pending_theme.borrow_mut().take();
        if let Some(name) = pending {
            let applied = apply_named_theme(context, &name);
            context.style_mut().scale_all_sizes(self.dpi_scale);
            scale_menu_item_padding(self.dpi_scale);
            *self.current_theme.borrow_mut() = applied.to_string();
        }
    }

    /// Name of the theme currently applied to this context.
    pub fn current_theme(&self) -> String {
        self.current_theme.borrow().clone()
    }

    pub fn draw_ui<F: FnOnce(&Ui)>(&self, delta_sec: f32, draw: F) -> ImguiFrame {
        self.update_delta_time(delta_sec);
        self.platform.borrow_mut().new_frame();

        let mut context = self.context.borrow_mut();
        // Apply any theme switch requested from the previous frame
        // before `frame()` snapshots the style for the new draw pass.
        self.drain_pending_theme(&mut context);
        let ui = context.frame();
        self.platform.borrow_mut().prepare_render(ui);

        draw(&ui);

        ImguiFrame { frame_begun: true }
    }

    pub fn context_mut(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
    }

    fn update_delta_time(&self, delta_sec: f32) {
        let mut context = self.context.borrow_mut();
        let io = context.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f32(delta_sec));
    }
}

#[cfg(not(vita))]
fn add_font_with_lucide(context: &mut Context, size_pixels: f32) {
    // imgui-rs flips `MergeMode` for every FontSource after the first
    // in a single `add_font` call. Listing Source Han Serif first and
    // Lucide second merges Lucide PUA glyphs into the same font slot,
    // so editor scripts can interpolate icons directly into widget
    // labels. `glyph_min_advance_x` keeps each icon a fixed square
    // next to the surrounding CJK text.
    context.fonts().add_font(&[
        FontSource::TtfData {
            data: radiance_assets::FONT_SOURCE_HAN_SERIF,
            size_pixels,
            config: Some(FontConfig {
                rasterizer_multiply: 1.75,
                glyph_ranges: FontGlyphRanges::chinese_full(),
                ..FontConfig::default()
            }),
        },
        FontSource::TtfData {
            data: radiance_assets::FONT_LUCIDE,
            size_pixels,
            config: Some(FontConfig {
                glyph_ranges: FontGlyphRanges::from_slice(&[0xE000, 0xF8FF, 0]),
                glyph_min_advance_x: size_pixels,
                ..FontConfig::default()
            }),
        },
    ]);
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
