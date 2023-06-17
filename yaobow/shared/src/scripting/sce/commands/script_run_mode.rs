use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandScriptRunMode {
    mode: i32,
}

impl SceCommand for SceCommandScriptRunMode {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state.set_run_mode(self.mode);
        return true;
    }
}

impl SceCommandScriptRunMode {
    pub fn new(mode: i32) -> Self {
        Self { mode }
    }
}
