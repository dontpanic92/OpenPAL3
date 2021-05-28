use crate::directors::sce_vm::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::{
    math::Vec3,
    scene::{Entity, SceneManager},
};

#[derive(Clone)]
pub struct SceCommandRoleFaceRole {
    role_id: i32,
    role_id2: i32,
}

impl SceCommand for SceCommandRoleFaceRole {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = scene_manager
            .get_resolved_role_entity(state, self.role_id)
            .transform()
            .position();
        let target_position = scene_manager
            .get_resolved_role_entity(state, self.role_id2)
            .transform()
            .position();

        scene_manager
            .get_resolved_role_entity_mut(state, self.role_id)
            .transform_mut()
            .look_at(&Vec3::new(target_position.x, position.y, target_position.z));
        return true;
    }
}

impl SceCommandRoleFaceRole {
    pub fn new(role_id: i32, role_id2: i32) -> Self {
        Self { role_id, role_id2 }
    }
}
