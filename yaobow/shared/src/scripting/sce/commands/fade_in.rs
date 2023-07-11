use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

pub const SCE_COMMAND_FADE_IN_TIMEOUT: f32 = 3.;

#[derive(Debug, Clone)]
pub struct SceCommandFadeIn {
    spent: f32
}

impl SceCommand for SceCommandFadeIn {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, _state: &mut SceState) {}

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let opacity = 1. - self.spent / SCE_COMMAND_FADE_IN_TIMEOUT;

        state.dialog_box().fade_window(ui, false, opacity);
        self.spent += delta_sec;

        self.spent >= SCE_COMMAND_FADE_IN_TIMEOUT
    }
}

impl SceCommandFadeIn {
    pub fn new() -> Self {
        Self {
            spent: 0.,
        }
    }
}
