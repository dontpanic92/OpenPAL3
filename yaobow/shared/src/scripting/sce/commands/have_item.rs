use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[allow(dead_code)]
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
        // TODO
        state.global_state_mut().fop_state_mut().push_value(true);
        true
    }
}

impl SceCommandHaveItem {
    pub fn new(item_id: i32) -> Self {
        Self { item_id }
    }
}
