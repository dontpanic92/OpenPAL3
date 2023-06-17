use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleCtrl {
    role_id: i32,
}

impl SceCommand for SceCommandRoleCtrl {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
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
