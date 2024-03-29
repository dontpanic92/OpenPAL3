use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

use crate::{
    openpal3::{directors::SceneManagerExtensions, scene::RoleController},
    scripting::sce::{SceCommand, SceState},
};

#[derive(Debug, Clone)]
pub struct SceCommandHyFly {
    position_x: f32,
    position_y: f32,
    position_z: f32,
}

impl SceCommand for SceCommandHyFly {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let entity = scene_manager.get_resolved_role(state, 5).unwrap();
        let role_controller = RoleController::get_role_controller(entity.clone()).unwrap();
        entity.transform().borrow_mut().set_position(&Vec3::new(
            self.position_x,
            self.position_y,
            self.position_z,
        ));
        role_controller.get().idle();
        true
    }
}

impl SceCommandHyFly {
    pub fn new(position_x: f32, position_y: f32, position_z: f32) -> Self {
        Self {
            position_x,
            position_y,
            position_z,
        }
    }
}
