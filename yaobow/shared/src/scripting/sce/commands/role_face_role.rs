use crate::openpal3::directors::SceneManagerExtensions;
use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct SceCommandRoleFaceRole {
    role_id: i32,
    role_id2: i32,
}

impl SceCommand for SceCommandRoleFaceRole {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = scene_manager
            .get_resolved_role(state, self.role_id)
            .and_then(|r| Some(r.transform().borrow().position()));
        let target_position = scene_manager
            .get_resolved_role(state, self.role_id2)
            .and_then(|r| Some(r.transform().borrow().position()));

        if position.is_none() {
            log::error!("Cannot find role {}", self.role_id);
            return true;
        }
        if target_position.is_none() {
            log::error!("Cannot find role {}", self.role_id2);
            return true;
        }

        scene_manager
            .get_resolved_role(state, self.role_id)
            .unwrap()
            .transform()
            .borrow_mut()
            .look_at(&Vec3::new(
                target_position.unwrap().x,
                position.unwrap().y,
                target_position.unwrap().z,
            ));
        true
    }
}

impl SceCommandRoleFaceRole {
    pub fn new(role_id: i32, role_id2: i32) -> Self {
        Self { role_id, role_id2 }
    }
}
