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
pub struct SceCommandRoleEndAction {
    role_id: i32,
}

impl SceCommand for SceCommandRoleEndAction {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        scene_manager
            .resolve_role_mut_do(state, self.role_id, |_, rc| {
                // A looping action (`RoleShowAction` mode `-1`) wraps forever and
                // never reaches `Idle`/`AnimationFinished`, so blocking on it here
                // would stall the script permanently. Mirror `RoleShowAction`'s
                // loop short-circuit: unblock immediately and leave the loop
                // playing (keeps the actor's current pose).
                if rc.repeat_mode() == RoleAnimationRepeatMode::Loop {
                    return true;
                }

                if rc.state() == RoleState::AnimationHolding {
                    rc.continue_anim();
                }

                rc.state() == RoleState::Idle || rc.state() == RoleState::AnimationFinished
            })
            .unwrap()
    }
}

impl SceCommandRoleEndAction {
    pub fn new(role_id: i32) -> Self {
        Self { role_id }
    }
}
