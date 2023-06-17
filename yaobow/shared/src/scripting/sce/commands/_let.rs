use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandLet {
    var: i16,
    value: i32,
}

impl SceCommand for SceCommandLet {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state
            .global_state_mut()
            .persistent_state_mut()
            .set_global(self.var, self.value);
        true
    }
}

impl SceCommandLet {
    pub fn new(var: i16, value: i32) -> Self {
        Self { var, value }
    }
}
