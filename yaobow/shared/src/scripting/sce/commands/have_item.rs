use crate::GameType;
use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandHaveItem {
    item_id: i32,
}

impl SceCommand for SceCommandHaveItem {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let has_item = state.asset_mgr().game() != GameType::PAL3
            || state
                .global_state_mut()
                .persistent_state()
                .inventory()
                .contains(self.item_id as u32);
        state
            .global_state_mut()
            .fop_state_mut()
            .push_value(has_item);
        true
    }
}

impl SceCommandHaveItem {
    pub fn new(item_id: i32) -> Self {
        Self { item_id }
    }
}
