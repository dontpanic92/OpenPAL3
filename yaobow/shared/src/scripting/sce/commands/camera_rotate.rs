use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::{Transform, Vec3};

#[derive(Debug, Clone)]
pub struct SceCommandCameraRotate {
    to_rot_x: f32,
    to_rot_y: f32,
    duration: f32,
    spent: f32,
    _unknown: i32,
}

impl SceCommand for SceCommandCameraRotate {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        _state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.scene().unwrap();
        let cam_rot = scene.camera().borrow().transform().euler();

        let left = (self.duration - self.spent).abs();
        let (x_rot, y_rot) = if left > Transform::EPS {
            let (mut x, mut y) = (
                (self.to_rot_x - cam_rot.x).to_radians() / left * _delta_sec,
                (self.to_rot_y - cam_rot.y).to_radians() / left * _delta_sec,
            );
            if x.abs() < Transform::EPS {
                x = 0.;
            }
            if y.abs() < Transform::EPS {
                y = 0.;
            }
            (x, y)
        } else {
            return true;
        };

        scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .rotate_axis_angle(&Vec3::UP, -x_rot)
            .rotate_axis_angle(&Vec3::EAST, -y_rot);

        self.spent += _delta_sec;
        self.spent > self.duration
    }
}

impl SceCommandCameraRotate {
    pub fn new(to_rot_x: f32, to_rot_y: f32, duration: f32, _unknown: i32) -> Self {
        Self {
            to_rot_x,
            to_rot_y,
            duration,
            spent: 0.,
            _unknown,
        }
    }
}
