use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandRoleInput {
    enable_input: i32,
}

impl SceCommand for SceCommandRoleInput {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
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
