use std::{cell::RefCell, rc::Rc};

use imgui::{Condition, MouseButton, Ui};
use radiance::{
    input::{InputEngine, Key},
    math::{Transform, Vec3},
    scene::{SceneManager, Viewport},
};

use crate::ui::window_content_rect;

pub struct SceneEditView {
    input: Rc<RefCell<dyn InputEngine>>,
    dragging: bool,
    start_transform: Transform,
}

impl SceneEditView {
    pub fn new(input: Rc<RefCell<dyn InputEngine>>) -> Self {
        Self {
            input,
            dragging: false,
            start_transform: Transform::new(),
        }
    }

    fn update_camera(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        self.dragging = ui.is_mouse_dragging(MouseButton::Left);

        if !self.dragging {
            self.start_transform = scene_manager
                .scene_mut()
                .unwrap()
                .camera_mut()
                .transform_mut()
                .clone()
        }

        let input = self.input.borrow();

        let z_translate = ui.io().mouse_wheel * -3000. * delta_sec
            + if input.get_key_state(Key::S).is_down() {
                1000. * delta_sec
            } else if input.get_key_state(Key::W).is_down() {
                -1000. * delta_sec
            } else {
                0.
            };

        // Commit the z translate regardingless whether we are dragging
        self.start_transform
            .translate_local(&Vec3::new(0., 0., z_translate));

        let mut transform = self.start_transform.clone();
        let [mouse_drag_x, mouse_drag_y] = ui.mouse_drag_delta();
        if self.dragging {
            transform.rotate_axis_angle(
                &Vec3::new(0., 1., 0.),
                mouse_drag_x * -0.002 * std::f32::consts::PI,
            );

            let matrix = transform.matrix();
            let yaw_axis = Vec3::new(matrix[0][0], matrix[1][0], matrix[2][0]);
            transform.rotate_axis_angle(&yaw_axis, mouse_drag_y * -0.002 * std::f32::consts::PI);
        }

        scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform_mut()
            .set_matrix(*transform.matrix());
    }

    fn update_scene(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        self.update_camera(scene_manager, ui, delta_sec);
    }
}

impl SceneEditView {
    pub fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("场景")
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.6, window_height * 0.7], Condition::Always)
            .position([window_width * 0.2, 0.], Condition::Always)
            .movable(false)
            .draw_background(false)
            .build(|| {
                let rect = window_content_rect(ui);
                scene_manager
                    .scene_mut()
                    .unwrap()
                    .camera_mut()
                    .set_viewport(Viewport::CustomViewport(rect));
            });

        self.update_scene(scene_manager, ui, delta_sec);
    }
}
