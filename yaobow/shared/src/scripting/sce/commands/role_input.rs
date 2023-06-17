use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleInput {
    enable_input: i32,
}

impl SceCommand for SceCommandRoleInput {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state
            .global_state_mut()
            .set_adv_input_enabled(self.enable_input != 0);
        true
    }
}

impl SceCommandRoleInput {
    pub fn new(enable_input: i32) -> Self {
        Self { enable_input }
    }
}
