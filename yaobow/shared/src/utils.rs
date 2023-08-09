use imgui::{Condition, Image, TextureId, Ui};
use radiance::rendering::VideoPlayer;

pub fn show_video_window(
    ui: &Ui,
    video_player: &mut VideoPlayer,
    texture_id: Option<TextureId>,
    window_size: [f32; 2],
    target_size: [f32; 2],
) -> Option<TextureId> {
    let mut ret_texture_id = None;
    ui.window("video")
        .size(window_size, Condition::Always)
        .position([0.0, 0.0], Condition::Always)
        .always_auto_resize(false)
        .draw_background(false)
        .scrollable(false)
        .no_decoration()
        .movable(false)
        .build(|| {
            if let Some(texture_id) = video_player.get_texture(texture_id) {
                ui.set_cursor_pos([
                    (window_size[0] - target_size[0]) * 0.5,
                    (window_size[1] - target_size[1]) * 0.5,
                ]);
                Image::new(texture_id, target_size).build(ui);
                ret_texture_id = Some(texture_id)
            }
        });

    ret_texture_id
}
