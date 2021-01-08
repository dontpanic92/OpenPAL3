use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandGoto {
    offset: i32,
}

impl SceCommand for SceCommandGoto {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.vm_context_mut().jump(self.offset);
        true
    }
}

impl SceCommandGoto {
    pub fn new(offset: i32) -> Self {
        Self { offset }
    }
}
