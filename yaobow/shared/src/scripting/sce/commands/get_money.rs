use crate::{
    GameType,
    scripting::sce::{SceCommand, SceState},
};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandGetMoney {
    var: i16,
}

impl SceCommand for SceCommandGetMoney {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let is_pal3 = state.asset_mgr().game() == GameType::PAL3;
        let money = if is_pal3 {
            state.global_state().persistent_state().money()
        } else {
            99999
        };
        if is_pal3 && self.var < 0 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .set_global(self.var, money);
        } else {
            state.context_mut().set_local(self.var, money);
        }
        true
    }
}

impl SceCommandGetMoney {
    pub fn new(var: i16) -> Self {
        Self { var }
    }
}
