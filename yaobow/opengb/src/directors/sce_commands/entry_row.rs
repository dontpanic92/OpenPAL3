use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

use super::SceCommandLoadScene;

#[derive(Debug, Clone)]
pub struct SceCommandEntryRow {
    id: i32,
    proc_id: i32,
}

impl SceCommand for SceCommandEntryRow {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let mut scene_loader = SceCommandLoadScene::new(
            match self.id {
                0 => "m04",
                1 => "m05",
                _ => panic!("explicit panic: unknown EntryRow id: {}", self.id),
            }
            .to_string(),
            "1".to_string(),
        );

        scene_loader.update(scene_manager, ui, state, delta_sec);
        state.call_proc(self.proc_id as u32);
        true
    }
}

impl SceCommandEntryRow {
    pub fn new(id: i32, proc_id: i32) -> Self {
        Self { id, proc_id }
    }
}
