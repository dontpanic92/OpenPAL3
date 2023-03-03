use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct SceCommandCameraMove {
    position: Vec3,
}

impl SceCommand for SceCommandCameraMove {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.scene().unwrap();
        scene
            .camera()
            .borrow_mut()
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
