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
            r.set_active(true);
            r.play_anim(&self.action_name, repeat);
        });
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        // A looping action (`-1`) never ends — it wraps continuously instead of
        // signalling `AnimationFinished` each cycle — so the script must not
        // block on it. Treat it as fire-and-forget and advance immediately.
        if self.repeat_mode == -1 {
            return true;
        }

        scene_manager
            .resolve_role_do(state, self.role_id, |_, r| {
                let s = r.state();
                s == RoleState::Idle
                    || s == RoleState::AnimationFinished
                    || s == RoleState::AnimationHolding
            })
            .unwrap()
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
