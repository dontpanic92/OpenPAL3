use crate::directors::sce_director::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Scene;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandCameraMove {
    position: Vec3,
}

impl SceCommand for SceCommandCameraMove {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.core_scene_mut_or_fail();
        scene
            .camera_mut()
            .transform_mut()
            .set_position(&self.position);

        return true;
    }
}

impl SceCommandCameraMove {
    pub fn new(
        position_x: f32,
        position_y: f32,
        position_z: f32,
        unknown_1: f32,
        unknown_2: f32,
    ) -> Self {
        Self {
            position: Vec3::new(position_x, position_y, position_z),
        }
    }
}
