use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandGoto {
    addr: u32,
}

impl SceCommand for SceCommandGoto {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state.context_mut().jump_to(self.addr);
        true
    }
}

impl SceCommandGoto {
    pub fn new(addr: u32) -> Self {
        Self { addr }
    }
}
