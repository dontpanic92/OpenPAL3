use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandStopMusic {}

impl SceCommand for SceCommandStopMusic {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
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
