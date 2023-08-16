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
            r if r > 0 => RoleAnimationRepeatMode::Repeat(r),
            -1 => RoleAnimationRepeatMode::Loop,
            -2 => RoleAnimationRepeatMode::Hold,
            _ => RoleAnimationRepeatMode::Repeat(1),
        };

        let _ = scene_manager.resolve_role_mut_do(state, self.role_id, |_, r| {
            r.get().set_active(true);
            r.get().play_anim(&self.action_name, repeat);
        });
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let rc = scene_manager.resolve_role_do(state, self.role_id, |_, r| r.get()).unwrap();
        let s = rc.state();

        s == RoleState::Idle || s == RoleState::AnimationFinished || s == RoleState::AnimationHolding
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
