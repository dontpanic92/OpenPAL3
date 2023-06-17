use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandGetAppr {
    var: i16,
}

impl SceCommand for SceCommandGetAppr {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if self.var < 0 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .set_global(self.var, 1)
        } else {
            state.context_mut().set_local(self.var, 1)
        }

        true
    }
}

impl SceCommandGetAppr {
    pub fn new(var: i16) -> Self {
        Self { var }
    }
}
