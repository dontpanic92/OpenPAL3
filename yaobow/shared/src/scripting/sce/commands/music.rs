use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandMusic {
    name: String,
}

impl SceCommand for SceCommandMusic {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if self.name.to_uppercase() == "NONE" {
            state.global_state_mut().bgm_source().stop();
        } else {
            state.global_state_mut().play_bgm(&self.name);
        }

        true
    }
}

impl SceCommandMusic {
    pub fn new(name: String, _unknown: i32) -> Self {
        Self { name }
    }
}
