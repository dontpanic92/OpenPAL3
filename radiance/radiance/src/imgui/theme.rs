//! Named imgui theme registry.
//!
//! Themes are embedded TOML files under `themes/`. `apply_theme(ctx, name)`
//! loads the named theme and overwrites the live `ImGuiStyle`. The parser is
//! tolerant: missing top-level keys and missing colors fall back to imgui's
//! defaults (or the most recently set value on the live style), so new
//! themes only need to override what they care about.

use imgui::{Context, Direction};
use std::cell::Cell;
use toml::{Table, Value};

thread_local! {
    /// Extra width (and unused `y`) added to dropdown menu popups, from
    /// the theme's `menuItemPadding`. `None` means "no extra width". This
    /// is a side-channel value because it is not part of `imgui::Style`;
    /// the menu renderer reads it via [`menu_item_padding`] to widen menu
    /// popups so their items' highlights extend past the label
    /// (Blender-style). It is reset on every [`apply_theme`] so a theme
    /// that omits `menuItemPadding` does not inherit a previous theme's
    /// value.
    static MENU_ITEM_PADDING: Cell<Option<[f32; 2]>> = const { Cell::new(None) };
}

/// Returns the DPI-scaled menu-item padding requested by the current theme,
/// or `None` if the theme does not override it. Consumed by the menu
/// renderer in `radiance_scripting`.
pub fn menu_item_padding() -> Option<[f32; 2]> {
    MENU_ITEM_PADDING.with(|p| p.get())
}

/// Multiply the stored menu-item padding by `scale`. Called after
/// `Style::scale_all_sizes` so this side-channel value is scaled the same
/// way imgui scales the built-in style fields.
pub fn scale_menu_item_padding(scale: f32) {
    MENU_ITEM_PADDING.with(|p| {
        if let Some([x, y]) = p.get() {
            p.set(Some([x * scale, y * scale]));
        }
    });
}

const YAOBOW: &str = include_str!("./themes/yaobow.toml");
const BLENDER_DARK: &str = include_str!("./themes/blender_dark.toml");
const BLENDER_LIGHT: &str = include_str!("./themes/blender_light.toml");

/// (name, embedded TOML). Order is the order shown in pickers.
/// The first entry is the fallback when an unknown name is requested.
const THEMES: &[(&str, &str)] = &[
    ("yaobow", YAOBOW),
    ("blender_dark", BLENDER_DARK),
    ("blender_light", BLENDER_LIGHT),
];

/// Default theme applied at `ImguiContext::new`. The game runtime relies on
/// this matching the pre-multitheme behaviour.
pub const DEFAULT_THEME: &str = "yaobow";

/// Names of every embedded theme, in display order.
pub fn available_themes() -> impl Iterator<Item = &'static str> {
    THEMES.iter().map(|(name, _)| *name)
}

/// Resolve `name` to a `'static` theme name from the registry,
/// falling back to [`DEFAULT_THEME`] on unknown names. Useful to
/// canonicalise a script-supplied name without actually applying it.
pub fn resolve_theme_name(name: &str) -> &'static str {
    theme_name_static(name).unwrap_or(DEFAULT_THEME)
}

/// Returns the embedded TOML source for `name`, or `None` if unknown.
fn theme_source(name: &str) -> Option<&'static str> {
    THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, src)| *src)
}

/// Apply the named theme to `context`. Falls back to [`DEFAULT_THEME`] if the
/// name is unknown. Returns the name that was actually applied.
///
/// The caller is responsible for re-running any DPI scaling on the style
/// after this call — see `ImguiContext::apply_theme`.
pub fn apply_theme(context: &mut Context, name: &str) -> &'static str {
    let (resolved, src) = match theme_source(name) {
        Some(src) => (theme_name_static(name).unwrap_or(DEFAULT_THEME), src),
        None => {
            log::warn!(
                "unknown imgui theme '{}', falling back to '{}'",
                name,
                DEFAULT_THEME
            );
            (DEFAULT_THEME, theme_source(DEFAULT_THEME).unwrap())
        }
    };

    let value = match src.parse::<Table>() {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to parse embedded theme '{}': {}", resolved, e);
            return resolved;
        }
    };

    apply_table(context, &value);
    resolved
}

fn theme_name_static(name: &str) -> Option<&'static str> {
    THEMES.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

fn apply_table(context: &mut Context, value: &Table) {
    // Reset the side-channel menu padding so a theme that omits the key
    // reverts to imgui's regular framePadding rather than inheriting the
    // previously applied theme's value.
    MENU_ITEM_PADDING.with(|p| p.set(get_array2(value, "menuItemPadding")));

    let style = context.style_mut();

    if let Some(v) = get_float(value, "alpha") {
        style.alpha = v;
    }
    if let Some(v) = get_float(value, "disabledAlpha") {
        style.disabled_alpha = v;
    }
    if let Some(v) = get_array2(value, "windowPadding") {
        style.window_padding = v;
    }
    if let Some(v) = get_float(value, "windowRounding") {
        style.window_rounding = v;
    }
    if let Some(v) = get_float(value, "windowBorderSize") {
        style.window_border_size = v;
    }
    if let Some(v) = get_array2(value, "windowMinSize") {
        style.window_min_size = v;
    }
    if let Some(v) = get_array2(value, "windowTitleAlign") {
        style.window_title_align = v;
    }
    if let Some(v) = get_direction(value, "windowMenuButtonPosition") {
        style.window_menu_button_position = v;
    }
    if let Some(v) = get_float(value, "childRounding") {
        style.child_rounding = v;
    }
    if let Some(v) = get_float(value, "childBorderSize") {
        style.child_border_size = v;
    }
    if let Some(v) = get_float(value, "popupRounding") {
        style.popup_rounding = v;
    }
    if let Some(v) = get_float(value, "popupBorderSize") {
        style.popup_border_size = v;
    }
    if let Some(v) = get_array2(value, "framePadding") {
        style.frame_padding = v;
    }
    if let Some(v) = get_float(value, "frameRounding") {
        style.frame_rounding = v;
    }
    if let Some(v) = get_float(value, "frameBorderSize") {
        style.frame_border_size = v;
    }
    if let Some(v) = get_array2(value, "itemSpacing") {
        style.item_spacing = v;
    }
    if let Some(v) = get_array2(value, "itemInnerSpacing") {
        style.item_inner_spacing = v;
    }
    if let Some(v) = get_array2(value, "cellPadding") {
        style.cell_padding = v;
    }
    if let Some(v) = get_float(value, "indentSpacing") {
        style.indent_spacing = v;
    }
    if let Some(v) = get_float(value, "columnsMinSpacing") {
        style.columns_min_spacing = v;
    }
    if let Some(v) = get_float(value, "scrollbarSize") {
        style.scrollbar_size = v;
    }
    if let Some(v) = get_float(value, "scrollbarRounding") {
        style.scrollbar_rounding = v;
    }
    if let Some(v) = get_float(value, "grabMinSize") {
        style.grab_min_size = v;
    }
    if let Some(v) = get_float(value, "grabRounding") {
        style.grab_rounding = v;
    }
    if let Some(v) = get_float(value, "tabRounding") {
        style.tab_rounding = v;
    }
    if let Some(v) = get_float(value, "tabBorderSize") {
        style.tab_border_size = v;
    }
    if let Some(v) = get_float(value, "tabMinWidthForCloseButton") {
        style.tab_min_width_for_close_button = v;
    }
    if let Some(v) = get_direction(value, "colorButtonPosition") {
        style.color_button_position = v;
    }
    if let Some(v) = get_array2(value, "buttonTextAlign") {
        style.button_text_align = v;
    }
    if let Some(v) = get_array2(value, "selectableTextAlign") {
        style.selectable_text_align = v;
    }

    if let Some(colors) = value.get("colors").and_then(Value::as_table) {
        apply_colors(style, colors);
    }
}

fn apply_colors(style: &mut imgui::Style, colors: &Table) {
    use imgui::sys::*;
    let mut set = |slot: u32, key: &str| {
        if let Some(c) = colors.get(key).and_then(parse_rgba) {
            style.colors[slot as usize] = c;
        }
    };

    set(ImGuiCol_Text, "Text");
    set(ImGuiCol_TextDisabled, "TextDisabled");
    set(ImGuiCol_WindowBg, "WindowBg");
    set(ImGuiCol_ChildBg, "ChildBg");
    set(ImGuiCol_PopupBg, "PopupBg");
    set(ImGuiCol_Border, "Border");
    set(ImGuiCol_BorderShadow, "BorderShadow");
    set(ImGuiCol_FrameBg, "FrameBg");
    set(ImGuiCol_FrameBgHovered, "FrameBgHovered");
    set(ImGuiCol_FrameBgActive, "FrameBgActive");
    set(ImGuiCol_TitleBg, "TitleBg");
    set(ImGuiCol_TitleBgActive, "TitleBgActive");
    set(ImGuiCol_TitleBgCollapsed, "TitleBgCollapsed");
    set(ImGuiCol_MenuBarBg, "MenuBarBg");
    set(ImGuiCol_ScrollbarBg, "ScrollbarBg");
    set(ImGuiCol_ScrollbarGrab, "ScrollbarGrab");
    set(ImGuiCol_ScrollbarGrabHovered, "ScrollbarGrabHovered");
    set(ImGuiCol_ScrollbarGrabActive, "ScrollbarGrabActive");
    set(ImGuiCol_CheckMark, "CheckMark");
    set(ImGuiCol_SliderGrab, "SliderGrab");
    set(ImGuiCol_SliderGrabActive, "SliderGrabActive");
    set(ImGuiCol_Button, "Button");
    set(ImGuiCol_ButtonHovered, "ButtonHovered");
    set(ImGuiCol_ButtonActive, "ButtonActive");
    set(ImGuiCol_Header, "Header");
    set(ImGuiCol_HeaderHovered, "HeaderHovered");
    set(ImGuiCol_HeaderActive, "HeaderActive");
    set(ImGuiCol_Separator, "Separator");
    set(ImGuiCol_SeparatorHovered, "SeparatorHovered");
    set(ImGuiCol_SeparatorActive, "SeparatorActive");
    set(ImGuiCol_ResizeGrip, "ResizeGrip");
    set(ImGuiCol_ResizeGripHovered, "ResizeGripHovered");
    set(ImGuiCol_ResizeGripActive, "ResizeGripActive");
    set(ImGuiCol_Tab, "Tab");
    set(ImGuiCol_TabHovered, "TabHovered");
    set(ImGuiCol_TabActive, "TabActive");
    set(ImGuiCol_TabUnfocused, "TabUnfocused");
    set(ImGuiCol_TabUnfocusedActive, "TabUnfocusedActive");
    set(ImGuiCol_PlotLines, "PlotLines");
    set(ImGuiCol_PlotLinesHovered, "PlotLinesHovered");
    set(ImGuiCol_PlotHistogram, "PlotHistogram");
    set(ImGuiCol_PlotHistogramHovered, "PlotHistogramHovered");
    set(ImGuiCol_TableHeaderBg, "TableHeaderBg");
    set(ImGuiCol_TableBorderStrong, "TableBorderStrong");
    set(ImGuiCol_TableBorderLight, "TableBorderLight");
    set(ImGuiCol_TableRowBg, "TableRowBg");
    set(ImGuiCol_TableRowBgAlt, "TableRowBgAlt");
    set(ImGuiCol_TextSelectedBg, "TextSelectedBg");
    set(ImGuiCol_DragDropTarget, "DragDropTarget");
    set(ImGuiCol_NavHighlight, "NavHighlight");
    set(ImGuiCol_NavWindowingHighlight, "NavWindowingHighlight");
    set(ImGuiCol_NavWindowingDimBg, "NavWindowingDimBg");
    set(ImGuiCol_ModalWindowDimBg, "ModalWindowDimBg");
}

fn get_float(table: &Table, key: &str) -> Option<f32> {
    let v = table.get(key)?;
    v.as_float()
        .map(|f| f as f32)
        .or_else(|| v.as_integer().map(|i| i as f32))
}

fn get_array2(table: &Table, key: &str) -> Option<[f32; 2]> {
    let arr = table.get(key)?.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let to_f = |v: &Value| -> Option<f32> {
        v.as_float()
            .map(|f| f as f32)
            .or_else(|| v.as_integer().map(|i| i as f32))
    };
    Some([to_f(&arr[0])?, to_f(&arr[1])?])
}

fn get_direction(table: &Table, key: &str) -> Option<Direction> {
    let s = table.get(key)?.as_str()?;
    Some(match s.to_lowercase().as_str() {
        "none" => Direction::None,
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => return None,
    })
}

fn parse_rgba(value: &Value) -> Option<[f32; 4]> {
    let s = value.as_str()?;
    let inner = s.strip_prefix("rgba(")?.strip_suffix(")")?;
    let parts: Vec<f32> = inner
        .split(',')
        .filter_map(|p| p.trim().parse::<f32>().ok())
        .collect();
    if parts.len() != 4 {
        return None;
    }
    Some([parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0, parts[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_theme_parses() {
        for (name, src) in THEMES {
            src.parse::<Table>()
                .unwrap_or_else(|e| panic!("theme '{}' failed to parse: {}", name, e));
        }
    }

    #[test]
    fn default_theme_is_listed() {
        assert!(available_themes().any(|n| n == DEFAULT_THEME));
    }

    #[test]
    fn parse_rgba_basic() {
        let v = Value::String("rgba(255, 128, 0, 1.0)".to_string());
        let c = parse_rgba(&v).unwrap();
        assert!((c[0] - 1.0).abs() < 1e-5);
        assert!((c[1] - 128.0 / 255.0).abs() < 1e-5);
        assert!((c[2] - 0.0).abs() < 1e-5);
        assert!((c[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn yaobow_defines_menu_item_padding() {
        let table = YAOBOW.parse::<Table>().unwrap();
        let pad = get_array2(&table, "menuItemPadding").expect("menuItemPadding present");
        // `x` is the extra dropdown popup width; it must be positive to
        // actually widen menu items.
        assert!(pad[0] > 0.0);
    }

    #[test]
    fn scale_menu_item_padding_scales_stored_value() {
        MENU_ITEM_PADDING.with(|p| p.set(Some([10.0, 4.0])));
        scale_menu_item_padding(2.0);
        assert_eq!(menu_item_padding(), Some([20.0, 8.0]));
        MENU_ITEM_PADDING.with(|p| p.set(None));
        scale_menu_item_padding(2.0);
        assert_eq!(menu_item_padding(), None);
    }
}
