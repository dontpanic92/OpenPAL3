use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandTestGoto {
    addr: u32,
}

impl SceCommand for SceCommandTestGoto {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if !state.global_state_mut().fop_state_mut().value().unwrap() {
            state.context_mut().jump_to(self.addr);
        }
        true
    }
}

impl SceCommandTestGoto {
    pub fn new(addr: u32) -> Self {
        Self { addr }
    }
}
