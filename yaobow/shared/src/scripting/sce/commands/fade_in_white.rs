use super::fade_in::SCE_COMMAND_FADE_IN_TIMEOUT;
use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandFadeInWhite {
    spent: f32,
}

impl SceCommand for SceCommandFadeInWhite {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let opacity = 1. - self.spent / SCE_COMMAND_FADE_IN_TIMEOUT;

        state.set_curtain(-opacity);
        self.spent += delta_sec;

        if self.spent >= SCE_COMMAND_FADE_IN_TIMEOUT {
            state.set_curtain(0.);
            true
        } else {
            false
        }
    }
}

impl SceCommandFadeInWhite {
    pub fn new() -> Self {
        Self { spent: 0. }
    }
}
