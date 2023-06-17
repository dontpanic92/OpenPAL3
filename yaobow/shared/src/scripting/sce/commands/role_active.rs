use crate::{
    openpal3::directors::SceneManagerExtensions,
    scripting::sce::{SceCommand, SceState},
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleActive {
    role_id: i32,
    active: i32,
}

impl SceCommand for SceCommandRoleActive {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        scene_manager.resolve_role_mut_do(state, self.role_id, |_e, r| {
            r.get().set_active(self.active != 0)
        });
        true
    }
}

impl SceCommandRoleActive {
    pub fn new(role_id: i32, active: i32) -> Self {
        Self { role_id, active }
    }
}
