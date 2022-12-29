use imgui::Ui;
use radiance::math::Rect;

pub mod scene_view;

pub fn window_content_rect(ui: &Ui) -> Rect {
    let [content_min_x, content_min_y] = ui.window_content_region_min();
    let [content_max_x, content_max_y] = ui.window_content_region_max();
    let [x, y] = ui.window_pos();
    let content_width = content_max_x - content_min_x;
    let content_height = content_max_y - content_min_y;
    let content_x = x + content_min_x;
    let content_y = y + content_min_y;

    Rect {
        x: content_x,
        y: content_y,
        width: content_width,
        height: content_height,
    }
}
