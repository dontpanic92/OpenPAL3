
use crate::directors::sce_director::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::{Entity, SceneManager};

#[derive(Clone)]
pub struct SceCommandRoleFaceRole {
    role_id: String,
    role_id2: String,
}

impl SceCommand for SceCommandRoleFaceRole {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.core_scene_mut_or_fail();
        let position = scene.get_role_entity(&self.role_id2).transform().position();
        scene
            .get_role_entity_mut(&self.role_id)
            .transform_mut()
            .look_at(&position);
        return true;
    }
}

impl SceCommandRoleFaceRole {
    pub fn new(role_id: i32, role_id2: i32) -> Self {
        Self {
            role_id: role_id.to_string(),
            role_id2: role_id2.to_string(),
        }
    }
}
