#[cfg_attr(any(android, vita), path = "clipboard_nop.rs")]
mod clipboard;

mod platform;
mod texture_resolver;
mod theme;

pub use self::texture_resolver::TextureResolver;
pub use self::theme::{DEFAULT_THEME, available_themes, menu_item_padding};

use self::theme::{apply_theme as apply_named_theme, resolve_theme_name, scale_menu_item_padding};

use crate::application::Platform;
use imgui::*;
use platform::ImguiPlatform;
use std::{
    cell::{Cell, RefCell, RefMut},
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

/// Size slots for the optional per-game font registered via
/// [`ImguiContext::add_game_font`]. They mirror the bundled
/// [`FontIndex::LARGE_FONT`] / [`FontIndex::SMALL_FONT`] sizes so in-game
/// widgets can pick a matching scale.
pub struct GameFontSize;
impl GameFontSize {
    pub const LARGE: usize = 0;
    pub const SMALL: usize = 1;
}

pub struct ImguiContext {
    context: Rc<RefCell<Context>>,
    platform: Rc<RefCell<ImguiPlatform>>,
    dpi_scale: f32,
    current_theme: RefCell<String>,
    pending_theme: RefCell<Option<String>>,
    /// Set when the font atlas has been mutated after the renderer built
    /// it (e.g. a game font was appended) and the GPU atlas texture needs
    /// to be rebuilt. Drained by the renderer via [`take_atlas_dirty`].
    atlas_dirty: Cell<bool>,
}

thread_local! {
    /// `FontId`s of the optional game-shipped font, one per
    /// [`GameFontSize`] slot. Kept in a thread-local (imgui runs on the
    /// single game thread) so in-game render sites that only hold an
    /// `imgui::Ui` — e.g. SCE dialog-selection commands — can reach the
    /// game font without threading a `UiManager` through. Empty until
    /// [`ImguiContext::add_game_font`] is called.
    static GAME_FONTS: RefCell<Vec<imgui::FontId>> = const { RefCell::new(Vec::new()) };
}

/// `FontId` of the registered game font for the given [`GameFontSize`]
/// slot, or `None` if no game font has been registered. Free function so
/// render sites with only an `imgui::Ui` can query it.
pub fn game_font(size: usize) -> Option<imgui::FontId> {
    GAME_FONTS.with(|f| f.borrow().get(size).copied())
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

        match clipboard::init() {
            Some(backend) => {
                context.set_clipboard_backend(backend);
            }
            _ => {
                log::error!("Failed to initialize clipboard support");
            }
        }

        let context = Rc::new(RefCell::new(context));
        let platform = ImguiPlatform::new(context.clone(), platform);
        Self {
            context,
            platform,
            dpi_scale,
            current_theme: RefCell::new(applied.to_string()),
            pending_theme: RefCell::new(None),
            atlas_dirty: Cell::new(false),
        }
    }

    /// Append a game-shipped font (raw TTF/TTC bytes) to the atlas as a
    /// large + small face pair, mirroring the bundled font sizes, and mark
    /// the atlas dirty so the renderer rebuilds the GPU texture. Replaces
    /// any previously registered game font. The `FontId`s are exposed via
    /// [`game_font`]; the bundled [`FontIndex`] slots are left untouched so
    /// the editor and title selector keep their font.
    ///
    /// The game font's pixel size is normalized so its ideographs render at
    /// the same visual height as the bundled font. imgui sizes fonts by
    /// their `ascender - descender` span, but CJK fonts differ widely in how
    /// much of that span their glyphs fill (e.g. SimSun's ideographs fill
    /// ~0.91 of the span vs Source Han Serif's ~0.64), so registering a game
    /// font at the bundled nominal size would render it noticeably larger.
    ///
    /// `data` only needs to outlive this call — imgui copies the bytes into
    /// the atlas. No-op on the Vita build, which uses a fixed bitmap font.
    #[cfg(not(vita))]
    pub fn add_game_font(&self, data: &[u8], extra_scale: f32) {
        let dpi = self.dpi_scale;
        let scale = game_font_size_scale(data) * extra_scale;
        log::info!(
            "Registering game font (size normalization scale {scale:.3}, extra {extra_scale:.3})"
        );
        let mut context = self.context.borrow_mut();
        let large = add_game_font_face(&mut context, data, 28. * dpi * scale);
        let small = add_game_font_face(&mut context, data, 18. * dpi * scale);
        GAME_FONTS.with(|f| *f.borrow_mut() = vec![large, small]);
        self.atlas_dirty.set(true);
    }

    #[cfg(vita)]
    pub fn add_game_font(&self, _data: &[u8], _extra_scale: f32) {}

    /// `FontId` of the registered game font for the given [`GameFontSize`]
    /// slot, or `None` if no game font has been registered.
    pub fn game_font(&self, size: usize) -> Option<imgui::FontId> {
        game_font(size)
    }

    /// Returns whether the font atlas needs a GPU rebuild and clears the
    /// flag. Called by the renderer once per frame.
    pub fn take_atlas_dirty(&self) -> bool {
        self.atlas_dirty.replace(false)
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
                // Source Han Serif renders slightly low within the line box,
                // so nudge it up a touch (negative y = up) for vertical
                // centering. Scales with the font size, which the caller has
                // already DPI-scaled.
                glyph_offset: [0.0, -size_pixels * 0.06],
                ..FontConfig::default()
            }),
        },
        FontSource::TtfData {
            data: radiance_assets::FONT_LUCIDE,
            size_pixels,
            config: Some(FontConfig {
                glyph_ranges: FontGlyphRanges::from_slice(&[0xE000, 0xF8FF, 0]),
                glyph_min_advance_x: size_pixels,
                // Lucide glyphs are authored on a metrics box whose baseline
                // sits higher than Source Han Serif, so without this nudge the
                // merged icons render above the surrounding text line. Push
                // them down by a fraction of the font size (scales for every
                // size add_font_with_lucide is called with, already DPI-scaled
                // by the caller) to share the text baseline.
                glyph_offset: [0.0, size_pixels * 0.12],
                ..FontConfig::default()
            }),
        },
    ]);
}

/// Ideograph-height ÷ vertical-span ratio of the bundled font
/// (`SourceHanSerif-Regular`), measured on the reference glyph 永 (U+6C38):
/// `glyph_bbox_height / (ascender - descender) = 911 / 1437`. Used as the
/// normalization target for game fonts (see [`game_font_size_scale`]).
/// Hard-coded rather than measured at runtime since the bundled font is
/// fixed. Recompute with `ttf-parser` if the bundled font is ever changed.
#[cfg(not(vita))]
const BUNDLED_IDEOGRAPH_RATIO: f32 = 0.634;

/// Scale factor to apply to a game font's nominal pixel size so its
/// ideographs render at the same visual height as the bundled font.
///
/// imgui/stb scales a font so that `ascender - descender` maps to the
/// requested `size_pixels`; the fraction of that span an ideograph fills
/// varies per font (e.g. SimSun ~0.90 vs the bundled font ~0.63), so a
/// same-`size_pixels` swap would render larger or smaller. We measure that
/// fraction for the game font and return [`BUNDLED_IDEOGRAPH_RATIO`] over
/// it. Falls back to `1.0` when the font can't be parsed, and clamps to a
/// sane range so a pathological face can't shrink or enlarge text
/// unreasonably.
#[cfg(not(vita))]
fn game_font_size_scale(game_data: &[u8]) -> f32 {
    match ideograph_span_ratio(game_data) {
        Some(g) if g > 0.0 => (BUNDLED_IDEOGRAPH_RATIO / g).clamp(0.5, 1.5),
        _ => 1.0,
    }
}

/// Ratio of the reference ideograph 永's height to the font's vertical
/// span (`ascender - descender`, the metric imgui/stb sizes by). 永 is the
/// canonical CJK metric glyph (it spans the full em box); a single
/// reference keeps the measurement consistent across faces — averaging or
/// taking the max over several glyphs skews fonts whose glyphs vary in
/// height (e.g. Kai). Falls back to a couple of other full-box glyphs only
/// if 永 is absent. Returns `None` if the font can't be parsed or has no
/// usable glyph.
#[cfg(not(vita))]
fn ideograph_span_ratio(data: &[u8]) -> Option<f32> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let span = (face.ascender() as i32 - face.descender() as i32) as f32;
    if span <= 0.0 {
        return None;
    }
    for ch in ['永', '国', '中'] {
        if let Some(gid) = face.glyph_index(ch) {
            if let Some(bbox) = face.glyph_bounding_box(gid) {
                let h = (bbox.y_max - bbox.y_min) as f32;
                if h > 0.0 {
                    return Some(h / span);
                }
            }
        }
    }
    None
}

/// Append a single game-font face at `size_pixels` to the atlas and return
/// its `FontId`. Loads only the CJK range (no Lucide merge — in-game text
/// doesn't use editor icons). `.ttc` collections load their first face,
/// which is the regular font we want (SimSun / MingLiU / Kai).
#[cfg(not(vita))]
fn add_game_font_face(context: &mut Context, data: &[u8], size_pixels: f32) -> imgui::FontId {
    context.fonts().add_font(&[FontSource::TtfData {
        data,
        size_pixels,
        config: Some(FontConfig {
            rasterizer_multiply: 1.75,
            glyph_ranges: FontGlyphRanges::chinese_full(),
            glyph_offset: [0.0, -size_pixels * 0.06],
            ..FontConfig::default()
        }),
    }])
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
