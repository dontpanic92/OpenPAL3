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

#[derive(Debug, Clone)]
pub struct SceCommandFullRoleAtt {
    role_id: i32,
    attribute: i32,
}

impl SceCommandFullRoleAtt {
    pub fn new(role_id: i32, attribute: i32) -> Self {
        Self { role_id, attribute }
    }
}

impl SceCommand for SceCommandFullRoleAtt {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if state.asset_mgr().game() == GameType::PAL3 {
            let data = state.asset_mgr().stats_data();
            if !state
                .global_state_mut()
                .persistent_state_mut()
                .restore_attribute(self.role_id, self.attribute, &data)
            {
                log::warn!(
                    "FullRoleAtt: invalid role/attribute {}/{}",
                    self.role_id,
                    self.attribute
                );
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct SceCommandFullTeamAtt;

impl SceCommandFullTeamAtt {
    pub fn new() -> Self {
        Self
    }
}

impl SceCommand for SceCommandFullTeamAtt {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if state.asset_mgr().game() == GameType::PAL3 {
            let data = state.asset_mgr().stats_data();
            state
                .global_state_mut()
                .persistent_state_mut()
                .restore_team_attributes(&data);
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct SceCommandAverageLv {
    role_id: i32,
    offset: i32,
}

impl SceCommandAverageLv {
    pub fn new(role_id: i32, offset: i32) -> Self {
        Self { role_id, offset }
    }
}

impl SceCommand for SceCommandAverageLv {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if state.asset_mgr().game() == GameType::PAL3 {
            let data = state.asset_mgr().stats_data();
            if !state
                .global_state_mut()
                .persistent_state_mut()
                .average_level(self.role_id, self.offset, &data)
            {
                log::warn!("AverageLv: invalid PAL3 party role {}", self.role_id);
            }
        }
        true
    }
}
