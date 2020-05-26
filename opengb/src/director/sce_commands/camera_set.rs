use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreScene, Scene};

#[derive(Clone)]
pub struct SceCommandCameraSet {
    y_rot: f32,
    x_rot: f32,
    position: Vec3,
}

impl SceCommand for SceCommandCameraSet {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let target = Vec3::add(
            &scene.camera().transform().position(),
            &Vec3::new(0., 0., -1.),
        );
        scene
            .camera_mut()
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
        unknown: f32,
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
