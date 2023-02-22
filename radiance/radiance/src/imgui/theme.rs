use std::convert::TryInto;

use imgui::{Context, Direction};
use toml::{Table, Value};

static THEME_STR: &'static str = include_str!("./theme.toml");

pub(crate) fn setup_theme(context: &mut Context) {
    let style = context.style_mut();
    let value = THEME_STR.parse::<Table>().unwrap();

    style.alpha = value["alpha"].as_float().unwrap() as f32;
    style.disabled_alpha = value["disabledAlpha"].as_float().unwrap() as f32;
    style.window_padding = to_array2(&value["windowPadding"]);
    style.window_rounding = value["windowRounding"].as_float().unwrap() as f32;
    style.window_border_size = value["windowBorderSize"].as_float().unwrap() as f32;
    style.window_min_size = to_array2(&value["windowMinSize"]);
    style.window_title_align = to_array2(&value["windowTitleAlign"]);
    style.window_menu_button_position = to_direction(&value["windowMenuButtonPosition"]);
    style.child_rounding = value["childRounding"].as_float().unwrap() as f32;
    style.child_border_size = value["childBorderSize"].as_float().unwrap() as f32;
    style.popup_rounding = value["popupRounding"].as_float().unwrap() as f32;
    style.popup_border_size = value["popupBorderSize"].as_float().unwrap() as f32;
    style.frame_padding = to_array2(&value["framePadding"]);
    style.frame_rounding = value["frameRounding"].as_float().unwrap() as f32;
    style.frame_border_size = value["frameBorderSize"].as_float().unwrap() as f32;
    style.item_spacing = to_array2(&value["itemSpacing"]);
    style.item_inner_spacing = to_array2(&value["itemInnerSpacing"]);
    style.cell_padding = to_array2(&value["cellPadding"]);
    style.indent_spacing = value["indentSpacing"].as_float().unwrap() as f32;
    style.columns_min_spacing = value["columnsMinSpacing"].as_float().unwrap() as f32;
    style.scrollbar_size = value["scrollbarSize"].as_float().unwrap() as f32;
    style.scrollbar_rounding = value["scrollbarRounding"].as_float().unwrap() as f32;
    style.grab_min_size = value["grabMinSize"].as_float().unwrap() as f32;
    style.grab_rounding = value["grabRounding"].as_float().unwrap() as f32;
    style.tab_rounding = value["tabRounding"].as_float().unwrap() as f32;
    style.tab_border_size = value["tabBorderSize"].as_float().unwrap() as f32;
    style.tab_min_width_for_close_button =
        value["tabMinWidthForCloseButton"].as_float().unwrap() as f32;
    style.color_button_position = to_direction(&value["colorButtonPosition"]);
    style.button_text_align = to_array2(&value["buttonTextAlign"]);
    style.selectable_text_align = to_array2(&value["selectableTextAlign"]);

    if let Value::Table(colors) = &value["colors"] {
        style.colors[imgui::sys::ImGuiCol_Text as usize] = color_str_to_array4(&colors["Text"]);
        style.colors[imgui::sys::ImGuiCol_TextDisabled as usize] =
            color_str_to_array4(&colors["TextDisabled"]);
        style.colors[imgui::sys::ImGuiCol_WindowBg as usize] =
            color_str_to_array4(&colors["WindowBg"]);
        style.colors[imgui::sys::ImGuiCol_ChildBg as usize] =
            color_str_to_array4(&colors["ChildBg"]);
        style.colors[imgui::sys::ImGuiCol_PopupBg as usize] =
            color_str_to_array4(&colors["PopupBg"]);
        style.colors[imgui::sys::ImGuiCol_Border as usize] = color_str_to_array4(&colors["Border"]);
        style.colors[imgui::sys::ImGuiCol_BorderShadow as usize] =
            color_str_to_array4(&colors["BorderShadow"]);
        style.colors[imgui::sys::ImGuiCol_FrameBg as usize] =
            color_str_to_array4(&colors["FrameBg"]);
        style.colors[imgui::sys::ImGuiCol_FrameBgHovered as usize] =
            color_str_to_array4(&colors["FrameBgHovered"]);
        style.colors[imgui::sys::ImGuiCol_FrameBgActive as usize] =
            color_str_to_array4(&colors["FrameBgActive"]);
        style.colors[imgui::sys::ImGuiCol_TitleBg as usize] =
            color_str_to_array4(&colors["TitleBg"]);
        style.colors[imgui::sys::ImGuiCol_TitleBgActive as usize] =
            color_str_to_array4(&colors["TitleBgActive"]);
        style.colors[imgui::sys::ImGuiCol_TitleBgCollapsed as usize] =
            color_str_to_array4(&colors["TitleBgCollapsed"]);
        style.colors[imgui::sys::ImGuiCol_MenuBarBg as usize] =
            color_str_to_array4(&colors["MenuBarBg"]);
        style.colors[imgui::sys::ImGuiCol_ScrollbarBg as usize] =
            color_str_to_array4(&colors["ScrollbarBg"]);
        style.colors[imgui::sys::ImGuiCol_ScrollbarGrab as usize] =
            color_str_to_array4(&colors["ScrollbarGrab"]);
        style.colors[imgui::sys::ImGuiCol_ScrollbarGrabHovered as usize] =
            color_str_to_array4(&colors["ScrollbarGrabHovered"]);
        style.colors[imgui::sys::ImGuiCol_ScrollbarGrabActive as usize] =
            color_str_to_array4(&colors["ScrollbarGrabActive"]);
        style.colors[imgui::sys::ImGuiCol_CheckMark as usize] =
            color_str_to_array4(&colors["CheckMark"]);
        style.colors[imgui::sys::ImGuiCol_SliderGrab as usize] =
            color_str_to_array4(&colors["SliderGrab"]);
        style.colors[imgui::sys::ImGuiCol_SliderGrabActive as usize] =
            color_str_to_array4(&colors["SliderGrabActive"]);
        style.colors[imgui::sys::ImGuiCol_Button as usize] = color_str_to_array4(&colors["Button"]);
        style.colors[imgui::sys::ImGuiCol_ButtonHovered as usize] =
            color_str_to_array4(&colors["ButtonHovered"]);
        style.colors[imgui::sys::ImGuiCol_ButtonActive as usize] =
            color_str_to_array4(&colors["ButtonActive"]);
        style.colors[imgui::sys::ImGuiCol_Header as usize] = color_str_to_array4(&colors["Header"]);
        style.colors[imgui::sys::ImGuiCol_HeaderHovered as usize] =
            color_str_to_array4(&colors["HeaderHovered"]);
        style.colors[imgui::sys::ImGuiCol_HeaderActive as usize] =
            color_str_to_array4(&colors["HeaderActive"]);
        style.colors[imgui::sys::ImGuiCol_Separator as usize] =
            color_str_to_array4(&colors["Separator"]);
        style.colors[imgui::sys::ImGuiCol_SeparatorHovered as usize] =
            color_str_to_array4(&colors["SeparatorHovered"]);
        style.colors[imgui::sys::ImGuiCol_SeparatorActive as usize] =
            color_str_to_array4(&colors["SeparatorActive"]);
        style.colors[imgui::sys::ImGuiCol_ResizeGrip as usize] =
            color_str_to_array4(&colors["ResizeGrip"]);
        style.colors[imgui::sys::ImGuiCol_ResizeGripHovered as usize] =
            color_str_to_array4(&colors["ResizeGripHovered"]);
        style.colors[imgui::sys::ImGuiCol_ResizeGripActive as usize] =
            color_str_to_array4(&colors["ResizeGripActive"]);
        style.colors[imgui::sys::ImGuiCol_Tab as usize] = color_str_to_array4(&colors["Tab"]);
        style.colors[imgui::sys::ImGuiCol_TabHovered as usize] =
            color_str_to_array4(&colors["TabHovered"]);
        style.colors[imgui::sys::ImGuiCol_TabActive as usize] =
            color_str_to_array4(&colors["TabActive"]);
        style.colors[imgui::sys::ImGuiCol_TabUnfocused as usize] =
            color_str_to_array4(&colors["TabUnfocused"]);
        style.colors[imgui::sys::ImGuiCol_TabUnfocusedActive as usize] =
            color_str_to_array4(&colors["TabUnfocusedActive"]);
        style.colors[imgui::sys::ImGuiCol_PlotLines as usize] =
            color_str_to_array4(&colors["PlotLines"]);
        style.colors[imgui::sys::ImGuiCol_PlotLinesHovered as usize] =
            color_str_to_array4(&colors["PlotLinesHovered"]);
        style.colors[imgui::sys::ImGuiCol_PlotHistogram as usize] =
            color_str_to_array4(&colors["PlotHistogram"]);
        style.colors[imgui::sys::ImGuiCol_PlotHistogramHovered as usize] =
            color_str_to_array4(&colors["PlotHistogramHovered"]);
        style.colors[imgui::sys::ImGuiCol_TableHeaderBg as usize] =
            color_str_to_array4(&colors["TableHeaderBg"]);
        style.colors[imgui::sys::ImGuiCol_TableBorderStrong as usize] =
            color_str_to_array4(&colors["TableBorderStrong"]);
        style.colors[imgui::sys::ImGuiCol_TableBorderLight as usize] =
            color_str_to_array4(&colors["TableBorderLight"]);
        style.colors[imgui::sys::ImGuiCol_TableRowBg as usize] =
            color_str_to_array4(&colors["TableRowBg"]);
        style.colors[imgui::sys::ImGuiCol_TableRowBgAlt as usize] =
            color_str_to_array4(&colors["TableRowBgAlt"]);
        style.colors[imgui::sys::ImGuiCol_TextSelectedBg as usize] =
            color_str_to_array4(&colors["TextSelectedBg"]);
        style.colors[imgui::sys::ImGuiCol_DragDropTarget as usize] =
            color_str_to_array4(&colors["DragDropTarget"]);
        style.colors[imgui::sys::ImGuiCol_NavHighlight as usize] =
            color_str_to_array4(&colors["NavHighlight"]);
        style.colors[imgui::sys::ImGuiCol_NavWindowingHighlight as usize] =
            color_str_to_array4(&colors["NavWindowingHighlight"]);
        style.colors[imgui::sys::ImGuiCol_NavWindowingDimBg as usize] =
            color_str_to_array4(&colors["NavWindowingDimBg"]);
        style.colors[imgui::sys::ImGuiCol_ModalWindowDimBg as usize] =
            color_str_to_array4(&colors["ModalWindowDimBg"]);
    }
}

fn to_array2(value: &Value) -> [f32; 2] {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_float().unwrap() as f32)
        .collect::<Vec<f32>>()
        .try_into()
        .unwrap()
}

fn to_direction(value: &Value) -> Direction {
    match value.as_str().unwrap().to_lowercase().as_str() {
        "none" => Direction::None,
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => Direction::None,
    }
}

fn color_str_to_array4(value: &Value) -> [f32; 4] {
    let s = value.as_str().unwrap();
    let s: String = s.chars().skip(5).take(s.len() - 5 - 1).collect();
    let mut output: Vec<f32> = s
        .split(",")
        .map(|f| f.trim().parse::<f32>().unwrap())
        .collect();
    output[0] /= 255.0;
    output[1] /= 255.0;
    output[2] /= 255.0;

    output.try_into().unwrap()
}
