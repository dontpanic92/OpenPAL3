use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::Scene;

#[derive(Clone)]
pub struct SceCommandCameraSet {
    y_rot: f32,
    x_rot: f32,
    position: Vec3,
}

impl SceCommand for SceCommandCameraSet {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene
            .camera_mut()
            .transform_mut()
            .set_position(&self.position)
            .rotate_axis_angle_local(&Vec3::UP, self.y_rot)
            .rotate_axis_angle_local(&Vec3::new(1., 0., 0.), self.x_rot);

        return true;
    }
}

impl SceCommandCameraSet {
    pub fn new(y_rot: f32, x_rot: f32, position: Vec3) -> Self {
        Self {
            y_rot,
            x_rot,
            position,
        }
    }
}
