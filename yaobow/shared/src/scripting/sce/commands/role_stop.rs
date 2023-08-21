use crate::{
    openpal3::directors::SceneManagerExtensions,
    scripting::sce::{SceCommand, SceState},
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleStop {
    role_id: i32,
}

impl SceCommand for SceCommandRoleStop {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let rc = scene_manager
            .resolve_role_mut_do(state, self.role_id, |_, r| r.get())
            .unwrap();

        rc.idle();

        true
    }
}

impl SceCommandRoleStop {
    pub fn new(role_id: i32) -> Self {
        Self { role_id }
    }
}
