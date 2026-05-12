use imgui::Ui;
use radiance::math::Rect;

pub mod scene_view;

pub fn window_content_rect(ui: &Ui) -> Rect {
    // Use GetCursorScreenPos + GetContentRegionAvail (the recommended replacement
    // for the deprecated GetWindowContentRegionMin/Max APIs, which produce
    // unreliable results inside docked windows in Dear ImGui >= 1.89.4).
    let (content_x, content_y, content_width, content_height) = unsafe {
        let mut cursor = imgui::sys::ImVec2::zero();
        imgui::sys::igGetCursorScreenPos(&mut cursor);
        let mut avail = imgui::sys::ImVec2::zero();
        imgui::sys::igGetContentRegionAvail(&mut avail);
        (cursor.x, cursor.y, avail.x, avail.y)
    };

    let _ = ui;

    Rect {
        x: content_x,
        y: content_y,
        width: content_width,
        height: content_height,
    }
}
