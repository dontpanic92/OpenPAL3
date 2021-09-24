use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandIfInTeam {
    role_id: i32,
}

impl SceCommand for SceCommandIfInTeam {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.global_state_mut().fop_state_mut().push_value(true);
        true
    }
}

impl SceCommandIfInTeam {
    pub fn new(role_id: i32) -> Self {
        Self { role_id }
    }
}
