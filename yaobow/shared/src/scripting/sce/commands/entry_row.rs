use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

use super::SceCommandLoadScene;

#[derive(Debug, Clone)]
pub struct SceCommandEntryRow {
    loader: super::SceCommandLoadScene,
    proc_id: i32,
}

impl SceCommand for SceCommandEntryRow {
    fn blocks_script(&self) -> bool {
        true
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if !self.loader.update(scene_manager, ui, state, delta_sec) {
            return false;
        }
        state.call_proc(self.proc_id as u32);
        true
    }
}

impl SceCommandEntryRow {
    pub fn new(id: i32, proc_id: i32) -> Self {
        let loader = SceCommandLoadScene::new(
            match id {
                0 => "m04",
                1 => "m05",
                _ => panic!("explicit panic: unknown EntryRow id: {}", id),
            }
            .to_string(),
            "1".to_string(),
        );

        Self { loader, proc_id }
    }
}
