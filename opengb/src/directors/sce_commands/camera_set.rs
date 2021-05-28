use crate::directors::sce_vm::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Scene;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandCameraSet {
    y_rot: f32,
    x_rot: f32,
    position: Vec3,
}

impl SceCommand for SceCommandCameraSet {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.core_scene_mut_or_fail();
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
