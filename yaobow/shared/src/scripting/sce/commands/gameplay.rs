use crate::{
    GameType,
    scripting::sce::{SceCommand, SceState},
};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandInTeam {
    role_id: i32,
    enabled: i32,
}

impl SceCommandInTeam {
    pub fn new(role_id: i32, enabled: i32) -> Self {
        Self { role_id, enabled }
    }
}

impl SceCommand for SceCommandInTeam {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if state.asset_mgr().game() == GameType::PAL3 {
            let valid = state
                .global_state_mut()
                .persistent_state_mut()
                .party_mut()
                .set_in_team(self.role_id, self.enabled != 0);
            if !valid {
                log::warn!("InTeam: invalid PAL3 party role {}", self.role_id);
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct SceCommandAddMoney {
    amount: i32,
}

impl SceCommandAddMoney {
    pub fn new(amount: i32) -> Self {
        Self { amount }
    }
}

impl SceCommand for SceCommandAddMoney {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if state.asset_mgr().game() == GameType::PAL3 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .add_money(self.amount);
        }
        true
    }
}
