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

pub fn play_movie(
    ui: &Ui,
    video_player: &mut VideoPlayer,
    texture_id: Option<TextureId>,
    source_size: (u32, u32),
    remove_black_bars: bool,
) -> Option<TextureId> {
    let window_size = ui.io().display_size;
    let (source_w, source_h) = source_size;

    // Keep aspect ratio
    let w_scale = window_size[0] / source_w as f32;
    let h_scale = if remove_black_bars {
        // Some of PAL3 movies are 4:3 ones with black bars on top and bottom
        // Scale movies to remove the black bars
        let new_source_h = source_w * 9 / 16;
        window_size[1] / new_source_h as f32
    } else {
        window_size[1] / source_h as f32
    };

    let scale = w_scale.min(h_scale);
    let target_size = [source_w as f32 * scale, source_h as f32 * scale];

    show_video_window(ui, video_player, texture_id, window_size, target_size)
}
