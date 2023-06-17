use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct SceCommandCameraSet {
    y_rot: f32,
    x_rot: f32,
    position: Vec3,
}

impl SceCommand for SceCommandCameraSet {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        _state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.scene().unwrap();
        let target = Vec3::add(
            &scene.camera().borrow().transform().position(),
            &Vec3::new(0., 0., -1.),
        );
        scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .look_at(&target)
            .set_position(&self.position)
            .rotate_axis_angle_local(&Vec3::UP, self.y_rot)
            .rotate_axis_angle_local(&Vec3::new(1., 0., 0.), self.x_rot);

        return true;
    }
}

impl SceCommandCameraSet {
    pub fn new(
        y_rot: f32,
        x_rot: f32,
        _unknown: f32,
        position_x: f32,
        position_y: f32,
        position_z: f32,
    ) -> Self {
        Self {
            y_rot: -y_rot.to_radians(),
            x_rot: x_rot.to_radians(),
            position: Vec3::new(position_x, position_y, position_z),
        }
    }
}
