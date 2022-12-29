use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleCtrl {
    role_id: i32,
}

impl SceCommand for SceCommandRoleCtrl {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.global_state_mut().set_role_controlled(self.role_id);
        true
    }
}

impl SceCommandRoleCtrl {
    pub fn new(role_id: i32) -> Self {
        Self { role_id }
    }
}
