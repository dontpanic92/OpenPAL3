use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandGetAppr {
    var: i16,
}

impl SceCommand for SceCommandGetAppr {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if self.var < 0 {
            state
                .shared_state_mut()
                .persistent_state_mut()
                .set_global(self.var, 1)
        } else {
            state.vm_context_mut().set_local(self.var, 1)
        }

        true
    }
}

impl SceCommandGetAppr {
    pub fn new(var: i16) -> Self {
        Self { var }
    }
}
