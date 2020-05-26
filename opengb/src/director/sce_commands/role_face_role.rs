use super::{map_role_id, SceneRoleExtensions};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::scene::{CoreScene, Entity};

#[derive(Clone)]
pub struct SceCommandRoleFaceRole {
    role_id: String,
    role_id2: String,
}

impl SceCommand for SceCommandRoleFaceRole {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = scene.get_role_entity(&self.role_id2).transform().position();
        scene
            .get_role_entity(&self.role_id)
            .transform_mut()
            .look_at(&position);
        return true;
    }
}

impl SceCommandRoleFaceRole {
    pub fn new(role_id: i32, role_id2: i32) -> Self {
        Self {
            role_id: map_role_id(role_id).to_string(),
            role_id2: map_role_id(role_id2).to_string(),
        }
    }
}
