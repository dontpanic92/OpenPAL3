use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;

use crate::{
    comdef::{IScene, ISceneExt},
    input::{InputEngine, Key},
    math::Vec3,
};

pub struct FreeViewController {
    input: Rc<RefCell<dyn InputEngine>>,
}

impl FreeViewController {
    pub fn new(input: Rc<RefCell<dyn InputEngine>>) -> Self {
        Self { input }
    }

    pub fn update(&self, scene: ComRc<IScene>, delta_sec: f32) {
        const SPEED: f32 = 500.0;

        let input = self.input.borrow_mut();

        {
            let mut camera = scene.camera_mut();
            let transform = camera.transform_mut();

            let mut movement = Vec3::new_zeros();
            if input.get_key_state(Key::W).is_down() {
                movement = Vec3::add(&movement, &Vec3::FRONT);
            }

            if input.get_key_state(Key::S).is_down() {
                movement = Vec3::sub(&movement, &Vec3::FRONT);
            }

            if input.get_key_state(Key::A).is_down() {
                movement = Vec3::sub(&movement, &Vec3::EAST);
            }

            if input.get_key_state(Key::D).is_down() {
                movement = Vec3::add(&movement, &Vec3::EAST);
            }

            if movement.norm() > 0.0 {
                movement.normalize();
                movement = Vec3::scalar_mul(SPEED * delta_sec, &movement);

                transform.translate_local(&movement);
            }

            let mut movement = Vec3::new(0., 0., 0.);

            if input.get_key_state(Key::Q).is_down() {
                movement = Vec3::add(&movement, &Vec3::UP);
            }

            if input.get_key_state(Key::E).is_down() {
                movement = Vec3::sub(&movement, &Vec3::UP);
            }

            if movement.norm() > 0.0 {
                movement.normalize();
                movement = Vec3::scalar_mul(SPEED * delta_sec, &movement);

                transform.translate(&movement);
            }

            const ROTATE_SPEED: f32 = 1.0;
            let mut yaw = 0.0f32;
            let mut pitch = 0.0f32;

            if input.get_key_state(Key::Left).is_down() {
                yaw += ROTATE_SPEED * delta_sec;
            }

            if input.get_key_state(Key::Right).is_down() {
                yaw -= ROTATE_SPEED * delta_sec;
            }

            if input.get_key_state(Key::Up).is_down() {
                pitch += ROTATE_SPEED * delta_sec;
            }

            if input.get_key_state(Key::Down).is_down() {
                pitch -= ROTATE_SPEED * delta_sec;
            }

            // Clamp pitch so the camera cannot flip over at the poles. The
            // current look-pitch is derived from the camera's forward axis
            // (its world-space y component) so it stays correct even if the
            // camera was reoriented externally between updates. A positive
            // local-EAST rotation drives forward_y negative, so negate to make
            // `current_pitch` increase together with a positive pitch delta.
            const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.01745; // ~89 deg
            let forward_y = transform.matrix()[1][2];
            let current_pitch = -forward_y.clamp(-1., 1.).asin();
            pitch = (current_pitch + pitch).clamp(-MAX_PITCH, MAX_PITCH) - current_pitch;

            // Yaw around the WORLD up axis and pitch around the camera's LOCAL
            // right axis. `rotate_axis_angle` (world space) also rotates the
            // translation column, which would orbit the camera around the world
            // origin. Capture the position first and restore it afterwards so
            // the yaw axis effectively passes through the camera's own position
            // (rotate in place). Pitch is a local rotation and leaves the
            // position untouched.
            let position = transform.position();
            transform
                .rotate_axis_angle(&Vec3::UP, yaw)
                .rotate_axis_angle_local(&Vec3::EAST, pitch);
            transform.set_position(&position);
        };
    }
}
