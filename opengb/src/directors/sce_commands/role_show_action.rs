use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use crate::scene::{RoleAnimationRepeatMode, RoleState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleShowAction {
    role_id: i32,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        let repeat = match self.repeat_mode {
            0 => RoleAnimationRepeatMode::Repeat,
            1 => RoleAnimationRepeatMode::NoRepeat,
            _ => RoleAnimationRepeatMode::NoRepeat,
        };

        let entity = scene_manager.resolve_role_mut_do(state, self.role_id, |entity| {
            entity.set_active(true);
            entity.play_anim(&self.action_name, repeat);
        });
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let s = scene_manager.resolve_role_do(state, self.role_id, |r| r.state());

        s.unwrap_or(RoleState::Idle) == RoleState::Idle
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
