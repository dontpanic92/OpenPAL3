use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandCall {
    proc_id: u32,
}

impl SceCommand for SceCommandCall {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.vm_context_mut().call_proc(self.proc_id);
        true
    }
}

impl SceCommandCall {
    pub fn new(proc_id: u32) -> Self {
        Self { proc_id }
    }
}
