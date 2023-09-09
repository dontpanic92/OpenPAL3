use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;

use crate::{
    comdef::IScene,
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
        const SPEED: f32 = 50.0;

        let input = self.input.borrow_mut();

        let camera = scene.camera();
        let mut camera = camera.borrow_mut();
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

        if input.get_key_state(Key::Q).is_down() {
            movement = Vec3::add(&movement, &Vec3::UP);
        }

        if input.get_key_state(Key::E).is_down() {
            movement = Vec3::sub(&movement, &Vec3::UP);
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

        if yaw < 0. {
            yaw += std::f32::consts::PI * 2.;
        }

        if yaw > std::f32::consts::PI * 2. {
            yaw -= std::f32::consts::PI * 2.;
        }

        if pitch < 0. {
            pitch += std::f32::consts::PI * 2.;
        }

        if pitch > std::f32::consts::PI * 2. {
            pitch -= std::f32::consts::PI * 2.;
        }

        if movement.norm() > 0.0 {
            movement.normalize();
            movement = Vec3::dot(SPEED * delta_sec, &movement);

            transform.translate_local(&movement);
        }

        transform
            .rotate_axis_angle_local(&Vec3::UP, yaw)
            .rotate_axis_angle_local(&Vec3::EAST, pitch);
    }
}
