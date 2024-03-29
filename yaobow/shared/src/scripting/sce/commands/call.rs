use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandCall {
    proc_id: u32,
}

impl SceCommand for SceCommandCall {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state.call_proc(self.proc_id);
        true
    }
}

impl SceCommandCall {
    pub fn new(proc_id: u32) -> Self {
        Self { proc_id }
    }
}
