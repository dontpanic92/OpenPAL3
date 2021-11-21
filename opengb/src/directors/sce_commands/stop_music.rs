use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandStopMusic {}

impl SceCommand for SceCommandStopMusic {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.global_state_mut().bgm_source().stop();
        state.global_state_mut().play_default_bgm();
        true
    }
}

impl SceCommandStopMusic {
    pub fn new() -> Self {
        Self {}
    }
}
