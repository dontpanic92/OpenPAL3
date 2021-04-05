use crate::directors::sce_director::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::{Entity, SceneManager};

#[derive(Clone)]
pub struct SceCommandRoleCtrl {
    role_id: i32,
}

impl SceCommand for SceCommandRoleCtrl {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.core_scene_mut_or_fail();

        // HACK: -1 should represent the current controlled role, not an actual role.
        let position = scene
            .get_role_entity(self.role_id)
            .transform()
            .position();
        state
            .shared_state_mut()
            .persistent_state_mut()
            .set_position(position);

        true
    }
}

impl SceCommandRoleCtrl {
    pub fn new(role_id: i32) -> Self {
        Self { role_id }
    }
}
