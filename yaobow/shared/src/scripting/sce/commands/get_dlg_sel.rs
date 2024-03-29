use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandGetDlgSel {
    var: i16,
}

impl SceCommand for SceCommandGetDlgSel {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let dlgsel = state.context_mut().current_proc_context_mut().get_dlgsel();
        state
            .context_mut()
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
