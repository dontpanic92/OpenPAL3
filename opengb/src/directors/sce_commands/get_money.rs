use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandGetMoney {
    var: i16,
}

impl SceCommand for SceCommandGetMoney {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state
            .context_mut()
            .current_proc_context_mut()
            .set_local(self.var, 99999);
        true
    }
}

impl SceCommandGetMoney {
    pub fn new(var: i16) -> Self {
        Self { var }
    }
}
