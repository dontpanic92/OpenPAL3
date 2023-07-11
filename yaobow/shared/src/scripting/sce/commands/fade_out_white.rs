use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use super::fade_in::SCE_COMMAND_FADE_IN_TIMEOUT;

#[derive(Debug, Clone)]
pub struct SceCommandFadeOutWhite {
    spent: f32
}

impl SceCommand for SceCommandFadeOutWhite {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, _state: &mut SceState) {}

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let opacity = self.spent / SCE_COMMAND_FADE_IN_TIMEOUT;

        state.dialog_box().fade_window(ui, true, opacity);
        self.spent += delta_sec;

        self.spent >= SCE_COMMAND_FADE_IN_TIMEOUT
    }
}

impl SceCommandFadeOutWhite {
    pub fn new() -> Self {
        Self {
            spent: 0.,
        }
    }
}
