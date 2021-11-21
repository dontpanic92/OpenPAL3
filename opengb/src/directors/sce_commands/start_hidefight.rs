use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandStartHideFight {}

impl SceCommand for SceCommandStartHideFight {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.call_proc(1701);
        true
    }
}

impl SceCommandStartHideFight {
    pub fn new() -> Self {
        Self {}
    }
}
