use crate::{
    openpal3::{
        directors::SceneManagerExtensions,
        scene::{RoleAnimationRepeatMode, RoleState},
    },
    scripting::sce::{SceCommand, SceState},
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleShowAction {
    role_id: i32,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn initialize(&mut self, scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        let repeat = match self.repeat_mode {
            0 => RoleAnimationRepeatMode::Repeat,
            1 => RoleAnimationRepeatMode::NoRepeat,
            _ => RoleAnimationRepeatMode::NoRepeat,
        };

        let _entity = scene_manager.resolve_role_mut_do(state, self.role_id, |_e, r| {
            r.get().set_active(true);
            r.get().play_anim(&self.action_name, repeat);
        });
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let s = scene_manager.resolve_role_do(state, self.role_id, |_e, r| r.get().state());

        let s = s.unwrap_or(RoleState::Idle);
        s == RoleState::Idle || s == RoleState::AnimationFinished
    }
}

impl SceCommandRoleShowAction {
    pub fn new(role_id: i32, action_name: String, repeat_mode: i32) -> Self {
        Self {
            role_id,
            action_name,
            repeat_mode,
        }
    }
}
