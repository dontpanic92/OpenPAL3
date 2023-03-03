use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandStartHideFight {}

impl SceCommand for SceCommandStartHideFight {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
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
