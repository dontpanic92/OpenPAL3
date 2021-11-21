use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandTestGoto {
    addr: u32,
}

impl SceCommand for SceCommandTestGoto {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
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
