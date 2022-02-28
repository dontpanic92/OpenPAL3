use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

use super::SceCommandDlgSel;

#[derive(Debug, Clone)]
pub struct SceCommandDlgTime {
    dlg_sel: SceCommandDlgSel,
}

impl SceCommand for SceCommandDlgTime {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        self.dlg_sel.update(scene_manager, ui, state, delta_sec)
    }
}

impl SceCommandDlgTime {
    pub fn new(text: String) -> Self {
        Self {
            dlg_sel: SceCommandDlgSel::new(vec![
                format!("2. \"{}\"", text),
                "1. \"……\"".to_string(),
            ]),
        }
    }
}
