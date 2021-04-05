use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandGetDlgSel {
    var: i16,
}

impl SceCommand for SceCommandGetDlgSel {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let dlgsel = state
            .vm_context_mut()
            .current_proc_context_mut()
            .get_dlgsel();
        state
            .vm_context_mut()
            .current_proc_context_mut()
            .set_local(self.var, dlgsel);
        true
    }
}

impl SceCommandGetDlgSel {
    pub fn new(var: i16) -> Self {
        Self { var }
    }
}
