use super::{map_role_id, SceneRoleExtensions};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::{RoleAnimationRepeatMode, RoleState, ScnScene};
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandRoleShowAction {
    role_id: String,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {
        let repeat = match self.repeat_mode {
            0 => RoleAnimationRepeatMode::Repeat,
            1 => RoleAnimationRepeatMode::NoRepeat,
            _ => RoleAnimationRepeatMode::NoRepeat,
        };

        scene
            .get_role_entity(&self.role_id)
            .play_anim(&self.action_name, repeat);
    }

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene.get_role_entity(&self.role_id).state() == RoleState::Idle
    }
}

impl SceCommandRoleShowAction {
    pub fn new(role_id: i32, action_name: String, repeat_mode: i32) -> Self {
        Self {
            role_id: map_role_id(role_id).to_string(),
            action_name,
            repeat_mode,
        }
    }
}
