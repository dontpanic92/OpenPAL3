use super::fade_in::SCE_COMMAND_FADE_IN_TIMEOUT;
use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandFadeOut {
    spent: f32,
}

impl SceCommand for SceCommandFadeOut {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let opacity = self.spent / SCE_COMMAND_FADE_IN_TIMEOUT;

        state.set_curtain(opacity);
        self.spent += delta_sec;

        if self.spent >= SCE_COMMAND_FADE_IN_TIMEOUT {
            state.set_curtain(1.);
            true
        } else {
            false
        }
    }
}

impl SceCommandFadeOut {
    pub fn new() -> Self {
        Self { spent: 0. }
    }
}
