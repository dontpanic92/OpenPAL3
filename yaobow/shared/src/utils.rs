use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::{Condition, Image, TextureId, Ui};
use radiance::{
    comdef::IScene,
    input::{Axis, InputEngine, Key},
    math::{Mat44, Vec3},
    rendering::VideoPlayer,
};

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

pub fn get_moving_direction(input: Rc<RefCell<dyn InputEngine>>, scene: ComRc<IScene>) -> Vec3 {
    let input = input.borrow_mut();
    let mut local_direction = Vec3::new(0., 0., 0.);

    if input.get_key_state(Key::Up).is_down() || input.get_key_state(Key::GamePadDPadUp).is_down() {
        local_direction = Vec3::add(&local_direction, &Vec3::new(0., 0., -1.));
    }

    if input.get_key_state(Key::Down).is_down()
        || input.get_key_state(Key::GamePadDPadDown).is_down()
    {
        local_direction = Vec3::add(&local_direction, &Vec3::new(0., 0., 1.));
    }

    local_direction = Vec3::add(
        &local_direction,
        &Vec3::new(0., 0., -input.get_axis_state(Axis::LeftStickY).value()),
    );

    if input.get_key_state(Key::Left).is_down()
        || input.get_key_state(Key::GamePadDPadLeft).is_down()
    {
        local_direction = Vec3::add(&local_direction, &Vec3::new(-1., 0., 0.));
    }

    if input.get_key_state(Key::Right).is_down()
        || input.get_key_state(Key::GamePadDPadRight).is_down()
    {
        local_direction = Vec3::add(&local_direction, &Vec3::new(1., 0., 0.));
    }

    local_direction = Vec3::add(
        &local_direction,
        &Vec3::new(input.get_axis_state(Axis::LeftStickX).value(), 0., 0.),
    );

    local_direction.normalize();

    let camera_mat = {
        let camera = scene.camera();
        let camera = camera.borrow();
        camera.transform().matrix().clone()
    };

    let mut world_direction_mat = Mat44::new_zero();
    world_direction_mat[0][3] = local_direction.x;
    world_direction_mat[1][3] = local_direction.y;
    world_direction_mat[2][3] = local_direction.z;
    let world_direction_mat = Mat44::multiplied(&camera_mat, &world_direction_mat);

    let world_direction = Vec3::new(world_direction_mat[0][3], 0., world_direction_mat[2][3]);
    Vec3::normalized(&world_direction)
}

pub fn get_camera_rotation(
    input: Rc<RefCell<dyn InputEngine>>,
    mut current_rotation: f32,
    delta_sec: f32,
) -> f32 {
    let input = input.borrow();
    const CAMERA_ROTATE_SPEED: f32 = 1.5;

    if input.get_key_state(Key::A).is_down() {
        current_rotation -= CAMERA_ROTATE_SPEED * delta_sec;
    }

    if input.get_key_state(Key::D).is_down() {
        current_rotation += CAMERA_ROTATE_SPEED * delta_sec;
    }

    current_rotation -=
        CAMERA_ROTATE_SPEED * delta_sec * input.get_axis_state(Axis::RightStickX).value();

    if current_rotation < 0. {
        current_rotation += std::f32::consts::PI * 2.;
    }

    if current_rotation > std::f32::consts::PI * 2. {
        current_rotation -= std::f32::consts::PI * 2.;
    }

    current_rotation
}
